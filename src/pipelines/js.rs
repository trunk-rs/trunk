//! JS asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::task::JoinHandle;

use super::{AssetFile, Attrs, TrunkAssetPipelineOutput, ATTR_SRC};
use crate::config::RtcBuild;
use crate::pipelines::AssetFileType;

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
        // Remove src and data-trunk from attributes.
        let attrs = attrs
            .into_iter()
            .filter(|(x, _)| *x != "src" && !x.starts_with("data-trunk"))
            .collect();
        Ok(Self {
            id,
            cfg,
            asset,
            attrs,
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
        tracing::info!(path = ?rel_path, "copying & hashing js");
        let file = self
            .asset
            .copy(
                &self.cfg.staging_dist,
                self.cfg.filehash,
                self.cfg.release,
                AssetFileType::Js,
            )
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing js");
        let attrs = Self::attrs_to_string(self.attrs);
        Ok(TrunkAssetPipelineOutput::Js(JsOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
            attrs,
        }))
    }

    /// Convert attributes to a string, to be used in JsOutput.
    fn attrs_to_string(attrs: Attrs) -> String {
        attrs
            .into_iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join(" ")
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
    pub attrs: String,
}

impl JsOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_script_id_selector(self.id))
            .replace_with_html(format!(
                r#"<script {attrs} src="{base}{file}"/>"#,
                attrs = self.attrs,
                base = &self.cfg.public_url,
                file = self.file
            ));
        Ok(())
    }
}
