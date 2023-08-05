//! Sass/Scss asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::{ok, BoxFuture};
use futures_util::stream::BoxStream;
use futures_util::FutureExt;
use nipper::Document;
use tokio::fs;
use tokio::task::JoinHandle;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::tools::Application;
use crate::util::{
    trunk_id_selector, Attrs, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_INLINE,
};

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
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The asset file being processed.
    asset: AssetFile,
    /// If the specified SASS/SCSS file should be inlined.
    use_inline: bool,
}

impl<C> Sass<C>
where
    C: SassConfig,
{
    pub const TYPE_SASS: &'static str = "sass";
    pub const TYPE_SCSS: &'static str = "scss";

    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr = attrs
            .get(ATTR_HREF)
            .with_reason(|| ErrorReason::PipelineLinkHrefNotFound { rel: "sass".into() })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        let use_inline = attrs.get(ATTR_INLINE).is_some();
        Ok(Self {
            id,
            cfg,
            asset,
            use_inline,
        })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(&self) -> Result<SassOutput<C>> {
        // tracing::info!("downloading sass");
        let version = self.cfg.version();
        let app = Application::SASS;

        let sass = app.get(version).await?;

        // Compile the target SASS/SCSS file.
        let style = if self.cfg.should_optimize() {
            "compressed"
        } else {
            "expanded"
        };
        let path_str = dunce::simplified(&self.asset.path).display().to_string();
        let file_name = format!("{}.css", &self.asset.file_stem.to_string_lossy());
        let file_path = dunce::simplified(&self.cfg.output_dir().join(&file_name))
            .display()
            .to_string();
        let args = &["--no-source-map", "-s", style, &path_str, &file_path];

        let rel_path = crate::util::strip_prefix(&self.asset.path);
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
        let css_ref = if self.use_inline {
            // Avoid writing any files, return the CSS as a String.
            CssRef::Inline(css)
        } else {
            // Hash the contents to generate a file name, and then write the contents to the dist
            // dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = self
                .cfg
                .should_hash()
                .then(|| format!("{}-{:x}.css", &self.asset.file_stem.to_string_lossy(), hash))
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

        tracing::info!(path = ?rel_path, "finished compiling sass/scss");
        Ok(SassOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            css_ref,
        })
    }
}

impl<C> Asset for Sass<C>
where
    C: 'static + SassConfig + Send + Sync,
{
    type Output = SassOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;
    type RunOnceFuture<'a> = BoxFuture<'a, Result<Self::Output>>;

    fn run_once(&self, input: super::AssetInput) -> Self::RunOnceFuture<'_> {
        self.run().boxed()
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<SassOutput<C>>> {
        tokio::spawn(async move { self.run().await })
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

impl<C> Output for SassOutput<C>
where
    C: SassConfig + Send + Sync,
{
    fn finalize<'life0, 'async_trait>(
        self,
        dom: &'life0 mut Document,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<()>> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
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
        ok(()).boxed()
    }
}
