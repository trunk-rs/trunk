//! Copy-file asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use indicatif::ProgressBar;
use nipper::Document;
use tokio::task::{spawn, JoinHandle};

use super::ATTR_HREF;
use super::{AssetFile, LinkAttrs, TrunkLinkPipelineOutput};
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

    pub async fn new(cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .ok_or_else(|| anyhow!(r#"required attr `href` missing for <link data-trunk rel="copyfile" .../> element"#))?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, progress, asset })
    }

    /// Spawn the pipeline for this asset type.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(async move {
            self.progress.set_message("copying file");
            let _ = self.asset.copy(&self.cfg.staging_dist).await?;
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
