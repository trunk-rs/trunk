//! Sass/Scss asset pipeline.

use super::{
    data_target_path, AssetFile, AttrWriter, Attrs, TrunkAssetPipelineOutput, ATTR_HREF,
    ATTR_INLINE, ATTR_NO_MINIFY,
};
use crate::{
    common::{self, dist_relative, html_rewrite::Document, nonce_attr, target_path},
    config::rt::RtcBuild,
    processing::integrity::{IntegrityType, OutputDigest},
    tools::{self, Application},
};
use anyhow::{ensure, Context, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, task::JoinHandle};

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
    /// E.g. `disabled`, `id="..."`
    other_attrs: Attrs,
    /// The required integrity setting
    integrity: IntegrityType,
    /// Whether to minify or not
    no_minify: bool,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl Sass {
    pub const TYPE_SASS: &'static str = "sass";
    pub const TYPE_SCSS: &'static str = "scss";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        attrs: Attrs,
        id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs.get(ATTR_HREF).context(
            r#"required attr `href` missing for <link data-trunk rel="sass|scss" .../> element"#,
        )?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        let use_inline = attrs.contains_key(ATTR_INLINE);
        let no_minify = attrs.contains_key(ATTR_NO_MINIFY);

        let integrity = IntegrityType::from_attrs(&attrs, &cfg)?;
        let target_path = data_target_path(&attrs)?;

        Ok(Self {
            id,
            cfg,
            asset,
            use_inline,
            other_attrs: attrs,
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
        let version = self.cfg.tools.sass.as_deref();
        let sass = tools::get(
            Application::Sass,
            version,
            self.cfg.offline,
            &self.cfg.client_options(),
        )
        .await?;

        let source_path_str = dunce::simplified(&self.asset.path).display().to_string();
        let source_test = common::path_exists_and(&source_path_str, |m| m.is_file()).await;
        ensure!(
            source_test.ok() == Some(true),
            "SASS source path '{source_path_str}' does not exist / is not a file"
        );

        let temp_target_file_name = format!("{}.css", &self.asset.file_stem.to_string_lossy());
        let temp_target_file_path =
            dunce::simplified(&self.cfg.staging_dist.join(&temp_target_file_name))
                .display()
                .to_string();

        // source map setting, embedded for non-release builds

        let source_map = match self.cfg.release {
            true => "--no-source-map",
            false => "--embed-source-map",
        };

        // put style, depends on minify state

        let output_style = match self.cfg.minify_asset(self.no_minify) {
            true => "compressed",
            false => "expanded",
        };

        // collect arguments

        let args = &[
            source_map,
            "--style",
            output_style,
            &source_path_str,
            &temp_target_file_path,
        ];

        // run

        let rel_path = common::strip_prefix(&self.asset.path);
        tracing::debug!(path = ?rel_path, "compiling sass/scss");
        common::run_command(
            Application::Sass.name(),
            &sass,
            args,
            &self.cfg.working_directory,
        )
        .await?;

        let css = fs::read_to_string(&temp_target_file_path)
            .await
            .with_context(|| format!("error reading CSS result file '{temp_target_file_path}'"))?;
        fs::remove_file(&temp_target_file_path).await?;

        // Check if the specified SASS/SCSS file should be inlined.
        let css_ref = if self.use_inline {
            // Avoid writing any files, return the CSS as a String.
            CssRef::Inline(css)
        } else {
            // Hash the contents to generate a file name, and then write the contents to the dist
            // dir.
            let hash = seahash::hash(css.as_bytes());

            let file_name = if self.cfg.filehash {
                format!("{}-{:x}.css", &self.asset.file_stem.to_string_lossy(), hash)
            } else {
                temp_target_file_name
            };

            let result_dir =
                target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;
            let file_path = result_dir.join(&file_name);
            let file_href = dist_relative(&self.cfg.staging_dist, &file_path)?;

            let integrity = OutputDigest::generate_from(self.integrity, css.as_bytes());

            // Write the generated CSS to the filesystem.
            fs::write(&file_path, css).await.with_context(|| {
                format!(
                    "error writing SASS pipeline output file '{}'",
                    file_path.display()
                )
            })?;

            // Generate a hashed reference to the new CSS file.
            CssRef::File(file_href, integrity)
        };

        tracing::debug!(path = ?rel_path, "finished compiling sass/scss");
        Ok(TrunkAssetPipelineOutput::Sass(SassOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            css_ref,
            attrs: self.other_attrs,
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
    pub css_ref: CssRef,
    /// The other attributes copied over from the original.
    pub attrs: Attrs,
}

/// The resulting CSS of the SASS/SCSS compilation.
pub enum CssRef {
    /// CSS to be inlined (for `data-inline`).
    Inline(String),
    /// A hashed file reference to a CSS file (default).
    File(String, OutputDigest),
}

impl SassOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let nonce = nonce_attr(&self.cfg.create_nonce);
        let html = match self.css_ref {
            // Insert the inlined CSS into a `<style>` tag.
            CssRef::Inline(css) => format!(
                r#"<style {attrs}{nonce}>{css}</style>"#,
                attrs = AttrWriter::new(&self.attrs, AttrWriter::EXCLUDE_CSS_INLINE)
            ),
            // Link to the CSS file.
            CssRef::File(file, integrity) => {
                let mut attrs = self.attrs.clone();
                integrity.insert_into(&mut attrs);

                format!(
                    r#"<link rel="stylesheet"{nonce} href="{base}{file}"{attrs}/>"#,
                    base = &self.cfg.public_url,
                    attrs = AttrWriter::new(&attrs, AttrWriter::EXCLUDE_CSS_LINK)
                )
            }
        };
        dom.replace_with_html(&super::trunk_id_selector(self.id), &html)
    }
}
