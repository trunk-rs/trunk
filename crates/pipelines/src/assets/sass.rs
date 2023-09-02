//! Sass/Scss asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream};
use futures_util::StreamExt;
use nipper::Document;
use tokio::fs;
use trunk_util::AssetInput;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::tools::Application;
use crate::util::{
    trunk_id_selector, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_INLINE, ATTR_REL,
};

static TYPE_SASS: &str = "sass";
static TYPE_SCSS: &str = "scss";

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
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_SASS)
            || input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_SCSS)
        {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        // Build the path to the target asset.
        let href_attr =
            input
                .attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: TYPE_SASS.into(),
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

/// A trait that indicates a type can be used as config type for sass pipeline.
pub trait SassConfig {
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

/// A sass/scss asset pipeline.
pub struct Sass<C> {
    /// Runtime build config.
    cfg: Arc<C>,
    inputs: Vec<Input>,
}

impl<C> Sass<C>
where
    C: SassConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            inputs: Vec::new(),
        }
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(cfg))]
    async fn run_with_input(cfg: Arc<C>, input: Input) -> Result<SassOutput<C>> {
        // tracing::info!("downloading sass");
        let version = cfg.version();
        let app = Application::SASS;

        let sass = app.get(version).await?;

        // Compile the target SASS/SCSS file.
        let style = if cfg.should_optimize() {
            "compressed"
        } else {
            "expanded"
        };
        let path_str = dunce::simplified(&input.file.path).display().to_string();
        let file_name = format!("{}.css", &input.file.file_stem.to_string_lossy());
        let file_path = dunce::simplified(&cfg.output_dir().join(&file_name))
            .display()
            .to_string();
        let args = &["--no-source-map", "-s", style, &path_str, &file_path];

        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "compiling sass/scss");
        sass.run_with_args(args).await?;

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

        // Check if the specified SASS/SCSS file should be inlined.
        let css_ref = if input.use_inline {
            // Avoid writing any files, return the CSS as a String.
            CssRef::Inline(css)
        } else {
            // Hash the contents to generate a file name, and then write the contents to the dist
            // dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = cfg
                .should_hash()
                .then(|| format!("{}-{:x}.css", &input.file.file_stem.to_string_lossy(), hash))
                .unwrap_or(file_name);
            let file_path = cfg.output_dir().join(&file_name);

            // Write the generated CSS to the filesystem.
            fs::write(&file_path, css)
                .await
                .with_reason(|| ErrorReason::FsWriteFailed {
                    path: file_path.to_owned(),
                })?;

            // Generate a hashed reference to the new CSS file.
            CssRef::File(file_name)
        };

        tracing::info!(path = ?rel_path, "finished compiling sass/scss");
        Ok(SassOutput {
            cfg,
            id: input.asset_input.id,
            css_ref,
        })
    }
}

#[async_trait]
impl<C> Asset for Sass<C>
where
    C: 'static + SassConfig + Send + Sync,
{
    type Output = SassOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        Self::run_with_input(self.cfg.clone(), input).await
    }

    fn outputs(self) -> Self::OutputStream {
        let Self { cfg, inputs } = self;

        stream::iter(inputs)
            .then(move |input| {
                let cfg = cfg.clone();
                tokio::spawn(async move { Self::run_with_input(cfg, input).await })
            })
            .map(|m| match m.reason(ErrorReason::TokioTaskFailed) {
                Ok(Ok(m)) => Ok(m),
                Ok(Err(e)) | Err(e) => Err(e),
            })
            .boxed()
    }
}

/// The output of a sass/scss build pipeline.
pub struct SassOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
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
    File(String),
}

#[async_trait(?Send)]
impl<C> Output for SassOutput<C>
where
    C: SassConfig + Send + Sync,
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
