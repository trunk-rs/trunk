//! Copy-dir asset pipeline.

use std::path::{Component, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::fs;
use tokio::task::JoinHandle;

use super::{Attrs, TrunkAssetPipelineOutput, ATTR_HREF};
use crate::common::copy_dir_recursive;
use crate::config::RtcBuild;

/// A CopyDir asset pipeline.
pub struct CopyDir {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The path to the dir being copied.
    path: PathBuf,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl CopyDir {
    pub const TYPE_COPY_DIR: &'static str = "copy-dir";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="copy-dir" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        if !path.is_absolute() {
            path = html_dir.join(path);
        }
        let target_path = attrs
            .get("data-target-path")
            .map(|val| val.parse())
            .transpose()?;

        Ok(Self {
            id,
            cfg,
            path,
            target_path,
        })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput>> {
        tokio::spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkAssetPipelineOutput> {
        let rel_path = crate::common::strip_prefix(&self.path);
        tracing::debug!(path = ?rel_path, "copying directory");

        let canonical_path = fs::canonicalize(&self.path).await.with_context(|| {
            format!("error taking canonical path of directory {:?}", &self.path)
        })?;
        let dir_name = canonical_path.file_name().with_context(|| {
            format!("could not get directory name of dir {:?}", &canonical_path)
        })?;

        let dir_out = if let Some(path) = self.target_path {
            if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
                anyhow::bail!(
                    "Invalid data-target-path '{}'. Must be a relative path without '..'.",
                    path.display()
                );
            }
            let dir_out = self.cfg.staging_dist.join(&path);
            tokio::fs::create_dir_all(&dir_out).await?;
            dir_out
        } else {
            self.cfg.staging_dist.join(dir_name)
        };
        copy_dir_recursive(canonical_path, dir_out).await?;

        tracing::debug!(path = ?rel_path, "finished copying directory");
        Ok(TrunkAssetPipelineOutput::CopyDir(CopyDirOutput(self.id)))
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
