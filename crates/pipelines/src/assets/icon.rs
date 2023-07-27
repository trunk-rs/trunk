//! Icon asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::ok;
use futures_util::FutureExt;
use nipper::Document;
use tokio::task::JoinHandle;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{trunk_id_selector, Attrs, ErrorReason, Result, ResultExt, ATTR_HREF};

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
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl<C> Icon<C>
where
    C: IconConfig,
{
    pub const TYPE_ICON: &'static str = "icon";

    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .with_reason(|| ErrorReason::PipelineLinkHrefNotFound { rel: "icon".into() })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, asset })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<IconOutput<C>> {
        let rel_path = crate::util::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying & hashing icon");
        let file = self
            .asset
            .copy(self.cfg.output_dir(), self.cfg.should_hash())
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing icon");
        Ok(IconOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
        })
    }
}

impl<C> Asset for Icon<C>
where
    C: 'static + IconConfig + Send + Sync,
{
    type Output = IconOutput<C>;

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<IconOutput<C>>> {
        tokio::spawn(self.run())
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

impl<C> Output for IconOutput<C>
where
    C: IconConfig + Send + Sync,
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
                r#"<link rel="icon" href="{base}{file}"/>"#,
                base = &self.cfg.public_url(),
                file = self.file
            ));
        ok(()).boxed()
    }
}
