//! CSS asset pipeline.

use super::{
    data_target_path, AssetFile, AttrWriter, Attrs, TrunkAssetPipelineOutput, ATTR_HREF,
    ATTR_NO_MINIFY,
};
use crate::{
    common::{html_rewrite::Document, target_path},
    config::rt::RtcBuild,
    pipelines::AssetFileType,
    processing::integrity::{IntegrityType, OutputDigest},
};
use anyhow::{Context, Result};
use std::path::PathBuf;
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
    /// Whether to minify or not
    no_minify: bool,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
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

        let integrity = IntegrityType::from_attrs(&attrs, &cfg)?;
        let no_minify = attrs.contains_key(ATTR_NO_MINIFY);
        let target_path = data_target_path(&attrs)?;

        Ok(Self {
            id,
            cfg,
            asset,
            attrs,
            integrity,
            no_minify,
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
        tracing::debug!(path = ?rel_path, "copying & hashing css");

        let result_path =
            target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;

        let file = self
            .asset
            .copy(
                &self.cfg.staging_dist,
                &result_path,
                self.cfg.filehash,
                self.cfg.minify_asset(self.no_minify),
                AssetFileType::Css,
            )
            .await?;
        tracing::debug!(path = ?rel_path, "finished copying & hashing css");

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

        dom.replace_with_html(
            &super::trunk_id_selector(self.id),
            &format!(
                r#"<link rel="stylesheet" href="{base}{file}"{attrs}/>"#,
                base = &self.cfg.public_url,
                file = self.file,
                attrs = AttrWriter::new(&attrs, AttrWriter::EXCLUDE_CSS_LINK),
            ),
        )
    }
}
