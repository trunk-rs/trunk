//! JS asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::task::JoinHandle;

use super::{AssetFile, TrunkAssetPipelineOutput};
use crate::config::RtcBuild;

/// A JS asset pipeline.
pub struct Js {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl Js {
    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        src: Option<String>,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let src_attr =
            src.context(r#"required attr `src` missing for <script data-trunk .../> element"#)?;
        let mut path = PathBuf::new();
        path.extend(src_attr.split('/'));
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
        tracing::info!(path = ?rel_path, "copying & hashing js");
        let file = self
            .asset
            .copy(&self.cfg.staging_dist, self.cfg.filehash)
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing js");
        Ok(TrunkAssetPipelineOutput::Js(JsOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
        }))
    }
}

/// The output of a JS build pipeline.
pub struct JsOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name of the finalized output file.
    pub file: String,
}

impl JsOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_script_id_selector(self.id))
            .replace_with_html(format!(
                r#"<script src="{base}{file}"/>"#,
                base = &self.cfg.public_url,
                file = self.file
            ));
        Ok(())
    }
}
