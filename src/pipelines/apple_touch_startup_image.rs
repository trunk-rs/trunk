//! AppleTouchStartupImage asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_std::task::{spawn, JoinHandle};
use nipper::Document;

use super::ATTR_HREF;
use super::{AssetFile, HashedFileOutput, LinkAttrs, TrunkLinkPipelineOutput};
use crate::config::RtcBuild;

/// An AppleTouchStartupImage asset pipeline.
pub struct AppleTouchStartupImage {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// Optional media attribute
    media: Option<String>,
}

impl AppleTouchStartupImage {
    pub const TYPE_APPLE_TOUCH_STARTUP_IMAGE: &'static str = "apple-touch-startup-image";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .context(r#"required attr `href` missing for <link data-trunk rel="apple-touch-startup-image" .../> element"#)?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        let media = attrs.get("media").map(|x| x.to_owned());
        Ok(Self { id, cfg, asset, media })
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
        tracing::info!(path = ?rel_path, "copying & hashing apple-touch-startup-image");
        let hashed_file_output = self.asset.copy_with_hash(&self.cfg.staging_dist).await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing apple-touch-startup-image");
        Ok(TrunkLinkPipelineOutput::AppleTouchStartupImage(AppleTouchStartupImageOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file: hashed_file_output,
            media: self.media,
        }))
    }
}

/// The output of an AppleTouchStartupImage build pipeline.
pub struct AppleTouchStartupImageOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Data on the finalized output file.
    pub file: HashedFileOutput,
    /// Optional media attribute
    pub media: Option<String>,
}

impl AppleTouchStartupImageOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let mut opts: Vec<String> = vec![];
        if let Some(media) = self.media {
            opts.push(format!("media=\"{}\"", media));
        }
        dom.select(&super::trunk_id_selector(self.id)).replace_with_html(format!(
            r#"<link rel="apple-touch-startup-image" href="{base}{file}" {optional}/>"#,
            base = &self.cfg.public_url,
            file = self.file.file_name,
            optional = opts.join(" "),
        ));
        Ok(())
    }
}
