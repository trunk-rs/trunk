//! Copy-dir asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::{ok, BoxFuture};
use futures_util::stream::BoxStream;
use futures_util::FutureExt;
use nipper::Document;
use tokio::fs;
use tokio::task::JoinHandle;

// #[cfg(test)]
// mod tests;
use super::{Asset, Output};
use crate::util::{
    copy_dir_recursive, trunk_id_selector, AssetInput, Error, ErrorReason, Result, ResultExt,
    ATTR_HREF,
};

static TYPE_COPY_DIR: &str = "copy-dir";

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,
    /// The path to the dir being copied.
    path: PathBuf,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl TryFrom<AssetInput> for Input {
    type Error = Error;

    fn try_from(value: AssetInput) -> std::result::Result<Self, Self::Error> {
        if value.attrs.get("rel").map(|m| m.as_str()) != Some(TYPE_COPY_DIR) {
            return Err(ErrorReason::AssetNotMatched { input: value }.into_error());
        }

        // Build the path to the target asset.
        let href_attr =
            value
                .attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: TYPE_COPY_DIR.into(),
                })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        if !path.is_absolute() {
            path = value.manifest_dir.join(path);
        }
        let target_path = value
            .attrs
            .get("data-target-path")
            .map(|m| Path::new(m).to_owned());

        Ok(Self {
            asset_input: value,
            path,
            target_path,
        })
    }
}

/// A trait that indicates a type can be used as config type for copy dir pipeline.
pub trait CopyDirConfig {
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;
}

/// A CopyDir asset pipeline.
#[derive(Debug)]
pub struct CopyDir<C> {
    /// Runtime build config.
    cfg: Arc<C>,
    /// Parsed inputs.
    inputs: Vec<Input>,
}

impl<C> CopyDir<C>
where
    C: CopyDirConfig,
{
    pub fn new(cfg: Arc<C>) -> Result<Self> {
        Ok(Self {
            cfg,
            inputs: Vec::new(),
        })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run_with_input(&self, input: Input) -> Result<CopyDirOutput> {
        let rel_path = crate::util::strip_prefix(&input.path);
        tracing::info!(path = ?rel_path, "copying directory");

        let canonical_path =
            fs::canonicalize(&input.path)
                .await
                .with_reason(|| ErrorReason::FsNotExist {
                    path: input.path.to_owned(),
                })?;
        let dir_name = canonical_path
            .file_name()
            .with_reason(|| ErrorReason::PathNoFileStem {
                path: canonical_path.to_owned(),
            })?;

        let out_rel_path = input
            .target_path
            .as_deref()
            .unwrap_or_else(|| dir_name.as_ref());

        let dir_out = self.cfg.output_dir().join(out_rel_path);

        if !dir_out.starts_with(self.cfg.output_dir()) {
            return Err(ErrorReason::PipelineLinkDataTargetPathRelativeExpected {
                path: out_rel_path.to_owned(),
            }
            .into_error());
        }

        copy_dir_recursive(canonical_path, dir_out).await?;

        tracing::info!(path = ?rel_path, "finished copying directory");
        Ok(CopyDirOutput(input.asset_input.id))
    }
}

#[async_trait]
impl<C> Asset for CopyDir<C>
where
    C: 'static + CopyDirConfig + Send + Sync,
{
    type Output = CopyDirOutput;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;
    type RunOnceFuture<'a> = BoxFuture<'a, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input)?;

        self.inputs.push(input);

        Ok(())
    }

    fn run_once(&self, input: super::AssetInput) -> Self::RunOnceFuture<'_> {
        async move {
            let input = Input::try_from(input)?;

            self.run_with_input(input).await
        }
        .boxed()
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }

    fn spawn(self) -> JoinHandle<Result<CopyDirOutput>> {
        todo!()
    }
}

/// The output of a CopyDir build pipeline.
pub struct CopyDirOutput(usize);

impl Output for CopyDirOutput {
    fn finalize<'life0, 'async_trait>(
        self,
        dom: &'life0 mut Document,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<()>> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        dom.select(&trunk_id_selector(self.0)).remove();
        ok(()).boxed()
    }
}
