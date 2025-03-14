//! Copy-file asset pipeline.

use crate::{
    common::{html_rewrite::Document, target_path},
    config::rt::RtcBuild,
    pipelines::{
        data_target_path, AssetFile, AssetFileType, Attrs, TrunkAssetPipelineOutput, ATTR_HREF,
    },
};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// A CopyFile asset pipeline.
pub struct CopyFile {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl CopyFile {
    pub const TYPE_COPY_FILE: &'static str = "copy-file";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="copy-file" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;

        let target_path = data_target_path(&attrs)?;

        Ok(Self {
            id,
            cfg,
            asset,
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
        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::debug!(path = ?rel_path, "copying file");

        let dir_out =
            target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;

        let _ = self
            .asset
            .copy(
                &self.cfg.staging_dist,
                &dir_out,
                false,
                false,
                AssetFileType::Other,
            )
            .await?;
        tracing::debug!(path = ?rel_path, "finished copying file");

        Ok(TrunkAssetPipelineOutput::CopyFile(CopyFileOutput(self.id)))
    }
}

/// The output of a CopyFile build pipeline.
pub struct CopyFileOutput(usize);

impl CopyFileOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.remove(&super::trunk_id_selector(self.0))
    }
}
