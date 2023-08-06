//! Icon asset pipeline.

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

static TYPE_ICON: &str = "icon";

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,

    /// The asset file being processed.
    file: AssetFile,
}

impl Input {
    async fn try_from(input: AssetInput) -> Result<Self> {
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_ICON) {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        // Build the path to the target asset.
        let href_attr = input
            .attrs
            .get(ATTR_HREF)
            .with_reason(|| ErrorReason::PipelineLinkHrefNotFound { rel: "icon".into() })?;
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

/// A trait that indicates a type can be used as config type for icon pipeline.
pub trait IconConfig {
    /// Returns the public url to be served.
    fn public_url(&self) -> &str;

    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;

    /// Returns true if the output file name should contain a file hash.
    fn should_hash(&self) -> bool;
}

/// An Icon asset pipeline.
pub struct Icon<C> {
    /// Runtime build config.
    cfg: Arc<C>,

    inputs: Vec<Input>,
}

impl<C> Icon<C>
where
    C: IconConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            inputs: Vec::new(),
        }
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(cfg))]
    async fn run_with_input(cfg: Arc<C>, input: Input) -> Result<IconOutput<C>> {
        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "copying & hashing icon");
        let file = input.file.copy(cfg.output_dir(), cfg.should_hash()).await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing icon");
        Ok(IconOutput {
            cfg,
            id: input.asset_input.id,
            file,
        })
    }
}

#[async_trait]
impl<C> Asset for Icon<C>
where
    C: 'static + IconConfig + Send + Sync,
{
    type Output = IconOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        Self::run_with_input(self.cfg.clone(), input).await
    }

    fn outputs(self) -> Self::OutputStream {
        let Self { cfg, inputs } = self;

        stream::iter(inputs.into_iter())
            .then(move |input| {
                let cfg = cfg.clone();
                tokio::spawn(async move { Self::run_with_input(cfg, input).await })
            })
            .map(|m| match m.reason(ErrorReason::TokioTaskFailed) {
                Ok(Ok(m)) => Ok(m),
                Ok(Err(e)) | Err(e) => Err(e),
            })
            .boxed()
    }
}

/// The output of an Icon build pipeline.
pub struct IconOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name of the finalized output file.
    pub file: String,
}

#[async_trait(?Send)]
impl<C> Output for IconOutput<C>
where
    C: IconConfig + Send + Sync,
{
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&trunk_id_selector(self.id))
            .replace_with_html(format!(
                r#"<link rel="icon" href="{base}{file}"/>"#,
                base = &self.cfg.public_url(),
                file = self.file
            ));

        Ok(())
    }
}
