//! Copy-dir asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_std::task::{spawn, JoinHandle};
use indicatif::ProgressBar;
use nipper::{Document, Selection};

use super::TrunkLinkPipelineOutput;
use super::ATTR_HREF;
use crate::common::copy_dir_recursive;
use crate::config::RtcBuild;

/// A CopyDir asset pipeline.
pub struct CopyDir {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The progress bar to use for this pipeline.
    progress: ProgressBar,
    /// The path to the dir being copied.
    path: PathBuf,
}

impl CopyDir {
    pub const TYPE_COPY_DIR: &'static str = "copy-dir";

    pub async fn new(cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, el: Selection<'_>, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = el
            .attr(ATTR_HREF)
            .ok_or_else(|| anyhow!("required attr `href` missing for <link data-trunk .../> element: {}", el.html()))?;
        let mut path = PathBuf::new();
        path.extend(href_attr.as_ref().split('/'));
        if !path.is_absolute() {
            path = html_dir.join(path);
        }
        Ok(Self { id, cfg, progress, path })
    }

    /// Spawn the pipeline for this asset type.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(async move {
            self.progress.set_message("copying directory");
            let canonical_path = async_std::path::Path::new(&self.path)
                .canonicalize()
                .await
                .with_context(|| format!("error taking canonical path of directory {:?}", &self.path))?;
            let dir_name = canonical_path
                .file_name()
                .ok_or_else(|| anyhow!("could not get directory name of dir {:?}", &canonical_path))?;
            let dir_out = self.cfg.staging_dist.join(dir_name);
            copy_dir_recursive(canonical_path.into(), dir_out).await?;
            self.progress.set_message("finished copying directory");
            Ok(TrunkLinkPipelineOutput::CopyDir(CopyDirOutput(self.id)))
        })
    }
}

/// The output of a CopyDir build pipeline.
pub struct CopyDirOutput(usize);

impl CopyDirOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.0)).remove();
        Ok(())
    }
}
