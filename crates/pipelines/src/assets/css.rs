//! CSS asset pipeline.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::ok;
use futures_util::stream::BoxStream;
use futures_util::FutureExt;
use nipper::Document;
use tokio::task::JoinHandle;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{trunk_id_selector, Attrs, ErrorReason, Result, ResultExt, ATTR_HREF};

/// A trait that indicates a type can be used as config type for css pipeline.
pub trait CssConfig {
    /// Returns the public url to be served.
    fn public_url(&self) -> &str;

    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;

    /// Returns true if the output file name should contain a file hash.
    fn should_hash(&self) -> bool;
}

/// A CSS asset pipeline.
pub struct Css<C> {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl<C> Css<C>
where
    C: 'static + CssConfig + Send + Sync,
{
    pub const TYPE_CSS: &'static str = "css";

    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .reason(ErrorReason::PipelineLinkHrefNotFound {
                rel: Cow::Borrowed("css"),
            })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, asset })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(&self) -> Result<CssOutput<C>> {
        let rel_path = crate::util::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying & hashing css");
        let file = self
            .asset
            .copy(self.cfg.output_dir(), self.cfg.should_hash())
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing css");
        Ok(CssOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
        })
    }
}

#[async_trait]
impl<C> Asset for Css<C>
where
    C: 'static + CssConfig + Send + Sync,
{
    type Output = CssOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn run_once(&self, input: super::AssetInput) -> Result<Self::Output> {
        self.run().await
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<CssOutput<C>>> {
        tokio::spawn(async move { self.run().await })
    }
}

/// The output of a CSS build pipeline.
pub struct CssOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name the finalized output file.
    pub file: String,
}

impl<C> Output for CssOutput<C>
where
    C: CssConfig + Send + Sync,
{
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
        dom.select(&trunk_id_selector(self.id))
            .replace_with_html(format!(
                r#"<link rel="stylesheet" href="{base}{file}"/>"#,
                base = &self.cfg.public_url(),
                file = self.file
            ));
        ok(()).boxed()
    }
}
