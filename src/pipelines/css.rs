//! CSS asset pipeline.

use super::{AssetFile, AttrWriter, Attrs, TrunkAssetPipelineOutput, ATTR_HREF, ATTR_INTEGRITY};
use crate::{
    config::RtcBuild,
    pipelines::AssetFileType,
    processing::integrity::{IntegrityType, OutputDigest},
};
use anyhow::{Context, Result};
use nipper::Document;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// A CSS asset pipeline.
pub struct Css {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// E.g. `disabled`, `id="..."`
    attrs: Attrs,
    /// The required integrity setting
    integrity: IntegrityType,
}

impl Css {
    pub const TYPE_CSS: &'static str = "css";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="css" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;

        let integrity = attrs
            .get(ATTR_INTEGRITY)
            .map(|value| IntegrityType::from_str(value))
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            id,
            cfg,
            asset,
            attrs,
            integrity,
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
        tracing::info!(path = ?rel_path, "copying & hashing css");
        let file = self
            .asset
            .copy(
                &self.cfg.staging_dist,
                self.cfg.filehash,
                self.cfg.release,
                AssetFileType::Css,
            )
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing css");

        let result_file = self.cfg.staging_dist.join(&file);
        let integrity = OutputDigest::generate(self.integrity, || std::fs::read(&result_file))
            .with_context(|| {
                format!(
                    "Failed to generate digest for CSS file '{}'",
                    result_file.display()
                )
            })?;

        Ok(TrunkAssetPipelineOutput::Css(CssOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
            other_attrs: self.attrs,
            integrity,
        }))
    }
}

/// The output of a CSS build pipeline.
pub struct CssOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name the finalized output file.
    pub file: String,
    /// The other attributes copied over from the original.
    pub other_attrs: Attrs,
    /// The digest for the integrity attribute
    pub integrity: OutputDigest,
}

impl CssOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let mut attrs = self.other_attrs.clone();

        self.integrity.insert_into(&mut attrs);

        dom.select(&super::trunk_id_selector(self.id))
            .replace_with_html(format!(
                r#"<link rel="stylesheet" href="{base}{file}"{attrs}/>"#,
                base = &self.cfg.public_url,
                file = self.file,
                attrs = AttrWriter::new(&attrs, AttrWriter::EXCLUDE_CSS_LINK),
            ));
        Ok(())
    }
}
