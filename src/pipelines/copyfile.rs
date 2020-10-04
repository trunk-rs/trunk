//! Copy-file asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_std::task::{spawn, JoinHandle};
use indicatif::ProgressBar;
use nipper::{Document, Selection};

use super::ATTR_HREF;
use super::{AssetFile, TrunkLinkPipelineOutput};
use crate::config::RtcBuild;

/// A CopyFile asset pipeline.
pub struct CopyFile {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The progress bar to use for this pipeline.
    progress: ProgressBar,
    /// The asset file being processed.
    asset: AssetFile,
}

impl CopyFile {
    pub const TYPE_COPY_FILE: &'static str = "copy-file";

    pub async fn new(cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, el: Selection<'_>, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = el
            .attr(ATTR_HREF)
            .ok_or_else(|| anyhow!("required attr `href` missing for <link data-trunk .../> element: {}", el.html()))?;
        let mut path = PathBuf::new();
        path.extend(href_attr.as_ref().split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, progress, asset })
    }

    /// Spawn the pipeline for this asset type.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(async move {
            self.progress.set_message("copying file");
            let _ = self.asset.copy(&self.cfg.dist).await?;
            self.progress.set_message("finished copying file");
            Ok(TrunkLinkPipelineOutput::CopyFile(CopyFileOutput(self.id)))
        })
    }
}

/// The output of a CopyFile build pipeline.
pub struct CopyFileOutput(usize);

impl CopyFileOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.0)).remove();
        Ok(())
    }
}
