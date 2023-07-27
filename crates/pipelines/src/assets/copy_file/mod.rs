//! Copy-file asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::ok;
use futures_util::FutureExt;
use nipper::Document;
use tokio::task::JoinHandle;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{trunk_id_selector, Attrs, ErrorReason, Result, ResultExt, ATTR_HREF};

#[cfg(test)]
mod tests;

/// A trait that indicates a type can be used as config type for copy file pipeline.
pub trait CopyFileConfig {
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;
}

/// A CopyFile asset pipeline.
pub struct CopyFile<C> {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl<C> CopyFile<C>
where
    C: CopyFileConfig,
{
    pub const TYPE_COPY_FILE: &'static str = "copy-file";

    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr =
            attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: "copy-file".into(),
                })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, asset })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<CopyFileOutput> {
        let rel_path = crate::util::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying file");
        let _ = self.asset.copy(self.cfg.output_dir(), false).await?;
        tracing::info!(path = ?rel_path, "finished copying file");
        Ok(CopyFileOutput(self.id))
    }
}

impl<C> Asset for CopyFile<C>
where
    C: 'static + CopyFileConfig + Send + Sync,
{
    type Output = CopyFileOutput;

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<CopyFileOutput>> {
        tokio::spawn(self.run())
    }
}

/// The output of a CopyFile build pipeline.
pub struct CopyFileOutput(usize);

impl Output for CopyFileOutput {
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
