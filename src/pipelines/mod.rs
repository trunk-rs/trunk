#[cfg(test)]
mod copy_dir_test;
mod copy_file;
#[cfg(test)]
mod copy_file_test;
mod html;
mod rust;

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use futures_util::future::ready;
use futures_util::TryFutureExt;
pub use html::HtmlPipeline;
use nipper::Document;
use serde::Deserialize;
use tokio::fs;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use trunk_pipelines::{
    CopyDir, CopyDirConfig, CopyDirOutput, Css, CssConfig, CssOutput, Icon, IconConfig, IconOutput,
    Inline, InlineOutput, Js, JsConfig, JsOutput, Output, Pipeline, Sass, SassConfig, SassOutput,
    TailwindCss, TailwindCssConfig, TailwindCssOutput,
};

use crate::common::path_exists;
use crate::config::RtcBuild;
use crate::pipelines::copy_file::{CopyFile, CopyFileOutput};
use crate::pipelines::rust::{RustApp, RustAppOutput};

impl JsConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl CssConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl SassConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }

    fn should_optimize(&self) -> bool {
        self.release
    }

    fn version(&self) -> Option<&str> {
        self.tools.sass.as_deref()
    }
}

impl IconConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl TailwindCssConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }

    fn should_optimize(&self) -> bool {
        self.release
    }

    fn version(&self) -> Option<&str> {
        self.tools.tailwindcss.as_deref()
    }
}

impl CopyDirConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

const ATTR_HREF: &str = "href";
const ATTR_REL: &str = "rel";
const SNIPPETS_DIR: &str = "snippets";
const TRUNK_ID: &str = "data-trunk-id";

/// A mapping of all attrs associated with a specific `<link data-trunk .../>` element.
pub type Attrs = HashMap<String, String>;

/// A reference to a trunk asset.
pub enum TrunkAssetReference {
    Link(Attrs),
    Script(Attrs),
}

/// A model of all of the supported Trunk asset links expressed in the source HTML as
/// `<trunk-link/>` elements.
///
/// Trunk will remove all `<trunk-link .../>` elements found in the HTML. It is the responsibility
/// of each pipeline to implement a pipeline finalizer method for its pipeline output in order to
/// update the finalized HTML for asset links and the like.
#[allow(clippy::large_enum_variant)]
pub enum TrunkAsset {
    Css(Css<RtcBuild>),
    Sass(Sass<RtcBuild>),
    TailwindCss(TailwindCss<RtcBuild>),
    Js(Js<RtcBuild>),
    Icon(Icon<RtcBuild>),
    Inline(Inline),
    CopyFile(CopyFile),
    CopyDir(CopyDir<RtcBuild>),
    RustApp(RustApp),
}

impl TrunkAsset {
    /// Construct a new instance.
    pub async fn from_html(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
        reference: TrunkAssetReference,
        id: usize,
    ) -> Result<Self> {
        match reference {
            TrunkAssetReference::Link(attrs) => {
                let rel = attrs.get(ATTR_REL).context(
                    "all <link data-trunk .../> elements must have a `rel` attribute indicating \
                     the asset type",
                )?;
                Ok(match rel.as_str() {
                    Sass::<RtcBuild>::TYPE_SASS | Sass::<RtcBuild>::TYPE_SCSS => {
                        Self::Sass(Sass::new(cfg, html_dir, attrs, id).await?)
                    }
                    Icon::<RtcBuild>::TYPE_ICON => {
                        Self::Icon(Icon::new(cfg, html_dir, attrs, id).await?)
                    }
                    Inline::TYPE_INLINE => Self::Inline(Inline::new(html_dir, attrs, id).await?),
                    Css::<RtcBuild>::TYPE_CSS => {
                        Self::Css(Css::new(cfg, html_dir, attrs, id).await?)
                    }
                    CopyFile::TYPE_COPY_FILE => {
                        Self::CopyFile(CopyFile::new(cfg, html_dir, attrs, id).await?)
                    }
                    CopyDir::<RtcBuild>::TYPE_COPY_DIR => {
                        Self::CopyDir(CopyDir::new(cfg, html_dir, attrs, id).await?)
                    }
                    RustApp::TYPE_RUST_APP => {
                        Self::RustApp(RustApp::new(cfg, html_dir, ignore_chan, attrs, id).await?)
                    }
                    TailwindCss::<RtcBuild>::TYPE_TAILWIND_CSS => {
                        Self::TailwindCss(TailwindCss::new(cfg, html_dir, attrs, id).await?)
                    }
                    _ => bail!(
                        r#"unknown <link data-trunk .../> attr value `rel="{}"`; please ensure the value is lowercase and is a supported asset type"#,
                        rel
                    ),
                })
            }
            TrunkAssetReference::Script(attrs) => {
                Ok(Self::Js(Js::new(cfg, html_dir, attrs, id).await?))
            }
        }
    }

    /// Spawn the build pipeline for this asset.
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput>> {
        // This is a workaround, the end result should be producing a type with a builder
        // pattern that processes each Output type recursively that can finalise the when all
        // pipelines are migrated.
        match self {
            Self::Css(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::Css)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::Sass(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::Sass)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::TailwindCss(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::TailwindCss)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::Js(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::Js)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::Icon(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::Icon)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::Inline(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::Inline)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::CopyFile(inner) => inner.spawn(),
            Self::CopyDir(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::CopyDir)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
            Self::RustApp(inner) => inner.spawn(),
        }
    }
}

/// The output of a `<trunk-link/>` asset pipeline.
pub enum TrunkAssetPipelineOutput {
    Css(CssOutput<RtcBuild>),
    Sass(SassOutput<RtcBuild>),
    TailwindCss(TailwindCssOutput<RtcBuild>),
    Js(JsOutput<RtcBuild>),
    Icon(IconOutput<RtcBuild>),
    Inline(InlineOutput),
    CopyFile(CopyFileOutput),
    CopyDir(CopyDirOutput),
    RustApp(RustAppOutput),
}

impl TrunkAssetPipelineOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        match self {
            TrunkAssetPipelineOutput::Css(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::Sass(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::TailwindCss(out) => {
                out.finalize(dom).await.map_err(|e| e.into())
            }
            TrunkAssetPipelineOutput::Js(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::Icon(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::Inline(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::CopyFile(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::CopyDir(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::RustApp(out) => out.finalize(dom).await,
        }
    }
}

/// An asset file to be processed by some build pipeline.
pub struct AssetFile {
    /// The canonicalized path to the target file.
    pub path: PathBuf,
    /// The name of the file itself.
    pub file_name: OsString,
    /// The file stem of the asset file.
    pub file_stem: OsString,
    /// The extension of the file.
    pub ext: Option<String>,
}

impl AssetFile {
    /// Create a new instance.
    ///
    /// The given path will be validated to ensure the following:
    /// - that the full canonicalized path points to a file on the FS.
    /// - that the file has a filename.
    /// - that the file has an extension.
    ///
    /// Any errors returned from this constructor indicate that one of these invariants was not
    /// upheld.
    pub async fn new(rel_dir: &Path, mut path: PathBuf) -> Result<Self> {
        // If the given path is not absolute, then we join it with the directory from which the
        // relative path should be based.
        if !path.is_absolute() {
            path = rel_dir.join(path);
        }

        // Take the path to referenced resource, if it is actually an FS path, then we continue.
        let path = fs::canonicalize(&path)
            .await
            .with_context(|| format!("error getting canonical path for {:?}", &path))?;
        ensure!(
            path_exists(&path).await?,
            "target file does not appear to exist on disk {:?}",
            &path
        );
        let file_name = match path.file_name() {
            Some(file_name) => file_name.to_owned(),
            None => bail!("asset has no file name {:?}", &path),
        };
        let file_stem = match path.file_stem() {
            Some(file_stem) => file_stem.to_owned(),
            None => bail!("asset has no file name stem {:?}", &path),
        };
        let ext = path
            .extension()
            .map(|ext| ext.to_owned().to_string_lossy().to_string());
        Ok(Self {
            path,
            file_name,
            file_stem,
            ext,
        })
    }

    /// Copy this asset to the target dir. If hashing is enabled, create a hash from the file
    /// contents and include it as hex string in the destination file name.
    ///
    /// The base file name (stripped path, without any parent folders) is returned if the operation
    /// was successful.
    pub async fn copy(&self, to_dir: &Path, with_hash: bool) -> Result<String> {
        let bytes = fs::read(&self.path)
            .await
            .with_context(|| format!("error reading file for copying {:?}", &self.path))?;

        let file_name = if with_hash {
            format!(
                "{}-{:x}.{}",
                &self.file_stem.to_string_lossy(),
                seahash::hash(bytes.as_ref()),
                &self.ext.as_deref().unwrap_or_default()
            )
        } else {
            self.file_name.to_string_lossy().into_owned()
        };

        let file_path = to_dir.join(&file_name);

        fs::write(&file_path, bytes)
            .await
            .with_context(|| format!("error copying file {:?} to {:?}", &self.path, &file_path))?;

        Ok(file_name)
    }
}

/// A stage in the build process.
///
/// This is used to specify when a hook will run.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    /// The stage before asset builds are executed.
    PreBuild,
    /// The stage where all asset builds are executed.
    Build,
    /// The stage after asset builds are executed.
    PostBuild,
}

/// Create the CSS selector for selecting a trunk link by ID.
pub(self) fn trunk_id_selector(id: usize) -> String {
    format!(r#"link[{}="{}"]"#, TRUNK_ID, id)
}
