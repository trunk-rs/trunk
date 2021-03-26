//! Sass/Scss asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_std::fs;
use async_std::task::{spawn, JoinHandle};
use nipper::Document;

use super::ATTR_HREF;
use super::{AssetFile, HashedFileOutput, LinkAttrs, TrunkLinkPipelineOutput};
use crate::common;
use crate::config::RtcBuild;
use crate::binary::{self, Application};

/// A sass/scss asset pipeline.
pub struct Sass {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
}

impl Sass {
    pub const TYPE_SASS: &'static str = "sass";
    pub const TYPE_SCSS: &'static str = "scss";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .context(r#"required attr `href` missing for <link data-trunk rel="sass|scss" .../> element"#)?;
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
        tracing::info!("downloading sass");
        let version = self.cfg.download.sass.as_deref();
        let sass = binary::get(Application::Sass, version).await?;

        // Compile the target SASS/SCSS file.
        let style = if self.cfg.release { "compressed" } else { "expanded" };
        let path_str = dunce::simplified(&self.asset.path).display().to_string();
        let file_name = format!("{}.css", &self.asset.file_stem.to_string_lossy());
        let file_path = dunce::simplified(&self.cfg.staging_dist.join(&file_name)).display().to_string();
        let args = &["--no-source-map", "-s", style, &path_str, &file_path];

        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "compiling sass/scss");
        common::run_command("sass", &sass, args).await?;

        // Hash the contents to generate a file name, and then write the contents to the dist dir.
        let css = fs::read(&file_path).await?;
        let hash = seahash::hash(&css);
        let hashed_file_name = format!("{}-{:x}.css", &self.asset.file_stem.to_string_lossy(), hash);
        let hashed_file_path = self.cfg.staging_dist.join(&hashed_file_name);
        fs::rename(file_path, &hashed_file_path).await?;

        tracing::info!(path = ?rel_path, "finished compiling sass/scss");
        Ok(TrunkLinkPipelineOutput::Sass(SassOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file: HashedFileOutput {
                hash,
                file_path: hashed_file_path,
                file_name: hashed_file_name,
            },
        }))
    }
}

/// The output of a sass/scss build pipeline.
pub struct SassOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Data on the finalized output file.
    pub file: HashedFileOutput,
}

impl SassOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.id)).replace_with_html(format!(
            r#"<link rel="stylesheet" href="{base}{file}"/>"#,
            base = &self.cfg.public_url,
            file = self.file.file_name
        ));
        Ok(())
    }
}
