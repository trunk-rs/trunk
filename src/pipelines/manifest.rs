//! Manifest asset pipeline (https://www.w3.org/TR/appmanifest/)

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_std::task::{spawn, JoinHandle};
use nipper::Document;

use super::ATTR_HREF;
use super::{AssetFile, HashedFileOutput, LinkAttrs, TrunkLinkPipelineOutput};
use crate::config::RtcBuild;

/// A manifest asset pipeline.
pub struct Manifest {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl Manifest {
    pub const TYPE_MANIFEST: &'static str = "manifest";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .context(r#"required attr `href` missing for <link data-trunk rel="manifest" .../> element"#)?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        Ok(Self { id, cfg, asset })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkLinkPipelineOutput> {
        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying & hashing manifest");
        let hashed_file_output = self.asset.copy_with_hash(&self.cfg.staging_dist).await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing manifest");
        Ok(TrunkLinkPipelineOutput::Manifest(ManifestOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file: hashed_file_output,
        }))
    }
}

/// The output of an Icon build pipeline.
pub struct ManifestOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Data on the finalized output file.
    pub file: HashedFileOutput,
}

impl ManifestOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.id)).replace_with_html(format!(
            r#"<link rel="manifest" href="{base}{file}"/>"#,
            base = &self.cfg.public_url,
            file = self.file.file_name
        ));
        Ok(())
    }
}
