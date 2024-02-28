//! Icon asset pipeline.

use super::{AssetFile, AttrWriter, Attrs, TrunkAssetPipelineOutput, ATTR_HREF};
use crate::common::target_path;
use crate::config::RtcBuild;
use crate::pipelines::{AssetFileType, ImageType};
use crate::processing::integrity::{IntegrityType, OutputDigest};
use anyhow::{Context, Result};
use nipper::Document;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// An Icon asset pipeline.
pub struct Icon {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// The required integrity setting
    integrity: IntegrityType,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl Icon {
    pub const TYPE_ICON: &'static str = "icon";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="icon" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;

        let integrity = IntegrityType::from_attrs(&attrs, &cfg)?;

        let target_path = attrs
            .get("data-target-path")
            .map(|val| val.parse())
            .transpose()?;

        Ok(Self {
            id,
            cfg,
            asset,
            integrity,
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
        tracing::debug!(path = ?rel_path, "copying & hashing icon");
        let mime_type = mime_guess::from_path(&self.asset.path).first_or_octet_stream();
        let image_type = match mime_type.type_().as_str() {
            "image/png" => ImageType::Png,
            _ => ImageType::Other,
        };

        let result_dir =
            target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;

        let file = self
            .asset
            .copy(
                &result_dir,
                self.cfg.filehash,
                self.cfg.release && !self.cfg.no_minification,
                AssetFileType::Icon(image_type),
            )
            .await?;

        let result_file = result_dir.join(&file);
        let integrity = OutputDigest::generate(self.integrity, || std::fs::read(&result_file))
            .with_context(|| {
                format!(
                    "Failed to generate digest for CSS file '{}'",
                    result_file.display()
                )
            })?;

        tracing::debug!(path = ?rel_path, "finished copying & hashing icon");
        Ok(TrunkAssetPipelineOutput::Icon(IconOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
            integrity,
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
    /// The digest for the integrity attribute
    pub integrity: OutputDigest,
}

impl IconOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let mut attrs = HashMap::new();
        self.integrity.insert_into(&mut attrs);

        dom.select(&super::trunk_id_selector(self.id))
            .replace_with_html(format!(
                r#"<link rel="icon" href="{base}{file}"{attrs}/>"#,
                base = &self.cfg.public_url,
                file = self.file,
                attrs = AttrWriter::new(&attrs, &[]),
            ));
        Ok(())
    }
}
