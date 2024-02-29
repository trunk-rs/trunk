//! JS asset pipeline.

use super::{
    data_target_path, AssetFile, AttrWriter, Attrs, TrunkAssetPipelineOutput, ATTR_MINIFY, ATTR_SRC,
};
use crate::common::target_path;
use crate::{
    config::RtcBuild,
    pipelines::AssetFileType,
    processing::integrity::{IntegrityType, OutputDigest},
};
use anyhow::{Context, Result};
use nipper::Document;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// A JS asset pipeline.
pub struct Js {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// The attributes to be placed on the output script tag.
    attrs: Attrs,
    /// The required integrity setting
    integrity: IntegrityType,
    /// If it's a JavaScript module (vs a classic script)
    module: bool,
    /// Whether to minify or not
    minify: bool,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl Js {
    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let src_attr = attrs
            .get(ATTR_SRC)
            .context(r#"required attr `src` missing for <script data-trunk .../> element"#)?;
        let mut path = PathBuf::new();
        path.extend(src_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;

        let integrity = IntegrityType::from_attrs(&attrs, &cfg)?;
        let module = attrs.get("type").map(|s| s.as_str()) == Some("module");
        let minify = attrs.get(ATTR_MINIFY).is_none();
        let target_path = data_target_path(&attrs)?;

        Ok(Self {
            id,
            cfg,
            asset,
            module,
            attrs,
            integrity,
            minify,
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
        tracing::debug!(path = ?rel_path, "copying & hashing js");
        let minify = self.cfg.release && self.minify && !self.cfg.no_minification;

        let result_dir =
            target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;

        let file = self
            .asset
            .copy(
                &self.cfg.staging_dist,
                &result_dir,
                self.cfg.filehash,
                minify,
                if self.module {
                    AssetFileType::Mjs
                } else {
                    AssetFileType::Js
                },
            )
            .await?;
        tracing::debug!(path = ?rel_path, file = ?file, "finished copying & hashing js");

        let result_file = self.cfg.staging_dist.join(&file);
        let integrity = OutputDigest::generate(self.integrity, || std::fs::read(&result_file))
            .with_context(|| {
                format!(
                    "Failed to generate digest for JS file '{}'",
                    result_file.display()
                )
            })?;

        Ok(TrunkAssetPipelineOutput::Js(JsOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
            attrs: self.attrs,
            integrity,
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
    /// The attributes to be added to the script tag.
    pub attrs: Attrs,
    /// The digest for the integrity attribute
    pub integrity: OutputDigest,
}

impl JsOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let mut attrs = self.attrs.clone();
        self.integrity.insert_into(&mut attrs);

        dom.select(&super::trunk_script_id_selector(self.id))
            .replace_with_html(format!(
                r#"<script src="{base}{file}"{attrs}/>"#,
                attrs = AttrWriter::new(&attrs, AttrWriter::EXCLUDE_SCRIPT),
                base = &self.cfg.public_url,
                file = self.file
            ));
        Ok(())
    }
}
