//! Tailwind CSS asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::BoxStream;
use nipper::Document;
use tokio::fs;
use trunk_util::AssetInput;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::tools::Application;
use crate::util::{
    trunk_id_selector, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_INLINE, ATTR_REL,
};

static TYPE_TAILWIND_CSS: &str = "tailwind-css";

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,
    /// The asset file being processed.
    file: AssetFile,
    /// If the specified SASS/SCSS file should be inlined.
    use_inline: bool,
}

impl Input {
    async fn try_from(input: AssetInput) -> Result<Self> {
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_TAILWIND_CSS) {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        // Build the path to the target asset.
        let href_attr =
            input
                .attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: "tailwind-css".into(),
                })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&input.manifest_dir, path).await?;
        let use_inline = input.attrs.get(ATTR_INLINE).is_some();

        let input = Input {
            asset_input: input,
            file: asset,
            use_inline,
        };

        Ok(input)
    }
}

/// A trait that indicates a type can be used as config type for tailwind css pipeline.
pub trait TailwindCssConfig {
    /// Returns the public url to be served.
    fn public_url(&self) -> &str;

    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;

    /// Returns true if the output file name should contain a file hash.
    fn should_hash(&self) -> bool;

    /// Returns the desired version for sass.
    fn version(&self) -> Option<&str>;

    /// Returns true if the final bundle should be optimised.
    fn should_optimize(&self) -> bool;
}

/// A tailwind css asset pipeline.
pub struct TailwindCss<C> {
    /// Runtime build config.
    cfg: Arc<C>,
    inputs: Vec<Input>,
}

impl<C> TailwindCss<C>
where
    C: TailwindCssConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            inputs: Vec::new(),
        }
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run_with_input(&self, input: Input) -> Result<TailwindCssOutput<C>> {
        let version = self.cfg.version();
        let app = Application::TAILWIND_CSS;
        let tailwind = app.get(version).await?;

        // Compile the target tailwind css file.
        let style = if self.cfg.should_optimize() {
            "--minify"
        } else {
            ""
        };
        let path_str = dunce::simplified(&input.file.path).display().to_string();
        let file_name = format!("{}.css", &input.file.file_stem.to_string_lossy());
        let file_path = dunce::simplified(&self.cfg.output_dir().join(&file_name))
            .display()
            .to_string();
        let args = &["--input", &path_str, "--output", &file_path, style];

        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "compiling tailwind css");
        tailwind.run_with_args(args).await?;

        let css =
            fs::read_to_string(&file_path)
                .await
                .with_reason(|| ErrorReason::FsReadFailed {
                    path: Path::new(file_path.as_str()).to_owned(),
                })?;
        fs::remove_file(&file_path)
            .await
            .with_reason(|| ErrorReason::FsRemoveFailed {
                path: Path::new(file_path.as_str()).to_owned(),
            })?;

        // Check if the specified tailwind css file should be inlined.
        let css_ref = if input.use_inline {
            // Avoid writing any files, return the CSS as a String.
            CssRef::Inline(css)
        } else {
            // Hash the contents to generate a file name, and then write the contents to the dist
            // dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = self
                .cfg
                .should_hash()
                .then(|| format!("{}-{:x}.css", &input.file.file_stem.to_string_lossy(), hash))
                .unwrap_or(file_name);
            let file_path = self.cfg.output_dir().join(&file_name);

            // Write the generated CSS to the filesystem.
            fs::write(&file_path, css)
                .await
                .with_reason(|| ErrorReason::FsWriteFailed {
                    path: file_path.to_owned(),
                })?;

            // Generate a hashed reference to the new CSS file.
            CssRef::File(file_name)
        };

        tracing::info!(path = ?rel_path, "finished compiling tailwind css");
        Ok(TailwindCssOutput {
            cfg: self.cfg.clone(),
            id: input.asset_input.id,
            css_ref,
        })
    }
}

#[async_trait]
impl<C> Asset for TailwindCss<C>
where
    C: 'static + TailwindCssConfig + Send + Sync,
{
    type Output = TailwindCssOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        self.run_with_input(input).await
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }
}

/// The output of a Tailwind CSS build pipeline.
pub struct TailwindCssOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Data on the finalized output file.
    pub css_ref: CssRef,
}

/// The resulting CSS of the Tailwind CSS compilation.
pub enum CssRef {
    /// CSS to be inlined (for `data-inline`).
    Inline(String),
    /// A hashed file reference to a CSS file (default).
    File(String),
}

#[async_trait(?Send)]
impl<C> Output for TailwindCssOutput<C>
where
    C: TailwindCssConfig + Send + Sync,
{
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        let html = match self.css_ref {
            // Insert the inlined CSS into a `<style>` tag.
            CssRef::Inline(css) => format!(r#"<style type="text/css">{}</style>"#, css),
            // Link to the CSS file.
            CssRef::File(file) => {
                format!(
                    r#"<link rel="stylesheet" href="{base}{file}"/>"#,
                    base = &self.cfg.public_url(),
                )
            }
        };
        dom.select(&trunk_id_selector(self.id))
            .replace_with_html(html);
        Ok(())
    }
}
