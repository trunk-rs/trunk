//! Sass/Scss asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use nipper::Document;
use tokio::fs;
use tokio::task::JoinHandle;

use super::{AssetFile, HashedFileOutput, LinkAttrs, TrunkLinkPipelineOutput};
use super::{ATTR_HREF, ATTR_INLINE};
use crate::common;
use crate::config::RtcBuild;
use crate::tools::{self, Application};

/// A sass/scss asset pipeline.
pub struct Sass {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The asset file being processed.
    asset: AssetFile,
    /// If the specified SASS/SCSS file should be inlined.
    use_inline: bool,
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
        let use_inline = attrs.get(ATTR_INLINE).is_some();
        Ok(Self { id, cfg, asset, use_inline })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        tokio::spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkLinkPipelineOutput> {
        // tracing::info!("downloading sass");
        let version = self.cfg.tools.sass.as_deref();
        let sass = tools::get(Application::Sass, version).await?;

        // Compile the target SASS/SCSS file.
        let style = if self.cfg.profile.is_some() || self.cfg.release {
            "compressed"
        } else {
            "expanded"
        };
        let path_str = dunce::simplified(&self.asset.path).display().to_string();
        let file_name = format!("{}.css", &self.asset.file_stem.to_string_lossy());
        let file_path = dunce::simplified(&self.cfg.staging_dist.join(&file_name))
            .display()
            .to_string();
        let args = &["--no-source-map", "-s", style, &path_str, &file_path];

        let rel_path = crate::common::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "compiling sass/scss");
        common::run_command("sass", &sass, args).await?;

        let css = fs::read_to_string(&file_path).await?;

        // Check if the specified SASS/SCSS file should be inlined.
        let css_ref = if self.use_inline {
            // Avoid writing any files, return the CSS as a String.
            CssRef::Inline(css)
        } else {
            // Hash the contents to generate a file name, and then write the contents to the dist dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = format!("{}-{:x}.css", &self.asset.file_stem.to_string_lossy(), hash);
            let file_path = self.cfg.staging_dist.join(&file_name);

            // Write the generated CSS to the filesystem.
            fs::write(&file_path, css)
                .await
                .context("error writing SASS pipeline output")?;

            // Generate a hashed reference to the new CSS file.
            CssRef::File(HashedFileOutput { hash, file_path, file_name })
        };

        tracing::info!(path = ?rel_path, "finished compiling sass/scss");
        Ok(TrunkLinkPipelineOutput::Sass(SassOutput { cfg: self.cfg.clone(), id: self.id, css_ref }))
    }
}

/// The output of a sass/scss build pipeline.
pub struct SassOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Data on the finalized output file.
    pub css_ref: CssRef,
}

/// The resulting CSS of the SASS/SCSS compilation.
pub enum CssRef {
    /// CSS to be inlined (for `data-inline`).
    Inline(String),
    /// A hashed file reference to a CSS file (default).
    File(HashedFileOutput),
}

impl SassOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let html = match self.css_ref {
            // Insert the inlined CSS into a `<style>` tag.
            CssRef::Inline(css) => format!(r#"<style type="text/css">{}</style>"#, css),
            // Link to the CSS file.
            CssRef::File(file) => {
                format!(
                    r#"<link rel="stylesheet" href="{base}{file}"/>"#,
                    base = &self.cfg.public_url,
                    file = file.file_name
                )
            }
        };
        dom.select(&super::trunk_id_selector(self.id)).replace_with_html(html);
        Ok(())
    }
}
