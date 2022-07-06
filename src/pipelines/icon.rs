//! Icon asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::task::JoinHandle;

use super::{AssetFile, LinkAttrs, TrunkAssetPipelineOutput, ATTR_HREF};
use crate::config::RtcBuild;

/// An Icon asset pipeline.
pub struct Icon {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl Icon {
    pub const TYPE_ICON: &'static str = "icon";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: LinkAttrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="icon" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, asset })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput>> {
        tokio::spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkAssetPipelineOutput> {
        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying & hashing icon");
        let file = self
            .asset
            .copy(&self.cfg.staging_dist, self.cfg.filehash)
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing icon");
        Ok(TrunkAssetPipelineOutput::Icon(IconOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
        }))
    }
}

/// The output of an Icon build pipeline.
pub struct IconOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name of the finalized output file.
    pub file: String,
}

impl IconOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.id))
            .replace_with_html(format!(
                r#"<link rel="icon" href="{base}{file}"/>"#,
                base = &self.cfg.public_url,
                file = self.file
            ));
        Ok(())
    }
}
