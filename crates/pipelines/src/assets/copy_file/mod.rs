//! Copy-file asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream};
use futures_util::StreamExt;
use nipper::Document;
use trunk_util::AssetInput;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{trunk_id_selector, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_REL};

// #[cfg(test)]
// mod tests;

static TYPE_COPY_FILE: &str = "copy-file";

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,
    /// The asset file being processed.
    file: AssetFile,
}

impl Input {
    async fn try_from(input: AssetInput) -> Result<Self> {
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_COPY_FILE) {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        // Build the path to the target asset.
        let href_attr =
            input
                .attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: TYPE_COPY_FILE.into(),
                })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&input.manifest_dir, path).await?;

        let input = Input {
            asset_input: input,
            file: asset,
        };

        Ok(input)
    }
}

/// A trait that indicates a type can be used as config type for copy file pipeline.
pub trait CopyFileConfig {
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;
}

/// A CopyFile asset pipeline.
#[derive(Debug)]
pub struct CopyFile<C> {
    /// Runtime build config.
    cfg: Arc<C>,

    inputs: Vec<Input>,
}

impl<C> CopyFile<C>
where
    C: CopyFileConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            inputs: Vec::new(),
        }
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(cfg))]
    async fn run_with_input(cfg: &C, input: &Input) -> Result<CopyFileOutput> {
        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "copying file");
        let _ = input.file.copy(cfg.output_dir(), false).await?;
        tracing::info!(path = ?rel_path, "finished copying file");
        Ok(CopyFileOutput(input.asset_input.id))
    }
}

#[async_trait]
impl<C> Asset for CopyFile<C>
where
    C: 'static + CopyFileConfig + Send + Sync,
{
    type Output = CopyFileOutput;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: super::AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        Self::run_with_input(self.cfg.as_ref(), &input).await
    }

    fn outputs(self) -> Self::OutputStream {
        let Self { cfg, inputs } = self;

        stream::iter(inputs.into_iter())
            .then(move |input| {
                let cfg = cfg.clone();
                tokio::spawn(async move { Self::run_with_input(cfg.as_ref(), &input).await })
            })
            .map(|m| match m.reason(ErrorReason::TokioTaskFailed) {
                Ok(Ok(m)) => Ok(m),
                Ok(Err(e)) | Err(e) => Err(e),
            })
            .boxed()
    }
}

/// The output of a CopyFile build pipeline.
pub struct CopyFileOutput(usize);

#[async_trait(?Send)]
impl Output for CopyFileOutput {
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&trunk_id_selector(self.0)).remove();
        Ok(())
    }
}
