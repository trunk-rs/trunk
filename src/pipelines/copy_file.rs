//! Copy-file asset pipeline.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::task::JoinHandle;

use super::ATTR_HREF;
use super::ATTR_RENAME;
use super::{AssetFile, LinkAttrs, TrunkLinkPipelineOutput};
use crate::config::RtcBuild;

/// A CopyFile asset pipeline.
pub struct CopyFile {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl CopyFile {
    pub const TYPE_COPY_FILE: &'static str = "copy-file";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .context(r#"required attr `href` missing for <link data-trunk rel="copy-file" .../> element"#)?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let mut asset = AssetFile::new(&html_dir, path).await?;
        // Check if the data-dist attribute has been used and update the asset accordingly.
        let optional: &String = &"[Optional Attribute Not Used]".to_owned();
        let new_name: &String = attrs.get(ATTR_RENAME).unwrap_or(optional);
        if new_name != optional {
            let mut new_stem: String = String::new();
            let mut new_ext: String = String::new();
            let mut post_dot: bool = false;
            // iterating in reverse for file names like trunk.copy_file.rs
            for character in new_name.chars().rev() {
                if character == '.' && !post_dot {
                    post_dot = true;
                    continue;
                }
                if post_dot {
                    new_stem.push(character);
                } else {
                    new_ext.push(character);
                }
            }
            asset.file_name = OsString::from(new_name);
            asset.file_stem = OsString::from(new_stem.chars().rev().collect::<String>());
            asset.ext = Some(new_ext.chars().rev().collect::<String>());
        }
        Ok(Self { id, cfg, asset })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        tokio::spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkLinkPipelineOutput> {
        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying file");
        let _ = self.asset.copy(&self.cfg.staging_dist).await?;
        tracing::info!(path = ?rel_path, "finished copying file");
        Ok(TrunkLinkPipelineOutput::CopyFile(CopyFileOutput(self.id)))
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
