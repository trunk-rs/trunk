mod copydir;
mod copyfile;
mod css;
mod html;
mod icon;
mod rust_app;
mod rust_worker;
mod sass;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use async_std::fs;
use async_std::task::JoinHandle;
use futures::channel::mpsc::Sender;
use indicatif::ProgressBar;
use nipper::{Document, Selection};

use crate::config::RtcBuild;
use crate::pipelines::copydir::{CopyDir, CopyDirOutput};
use crate::pipelines::copyfile::{CopyFile, CopyFileOutput};
use crate::pipelines::css::{Css, CssOutput};
use crate::pipelines::icon::{Icon, IconOutput};
use crate::pipelines::rust_app::{RustApp, RustAppOutput};
use crate::pipelines::rust_worker::{RustWorker, RustWorkerOutput};
use crate::pipelines::sass::{Sass, SassOutput};

pub use html::HtmlPipeline;

const ATTR_HREF: &str = "href";
const ATTR_REL: &str = "rel";
const SNIPPETS_DIR: &str = "snippets";
const TRUNK_ID: &str = "data-trunk-id";

/// A model of all of the supported Trunk asset links expressed in the source HTML as
/// `<trunk-link/>` elements.
///
/// Trunk will remove all `<trunk-link .../>` elements found in the HTML. It is the responsibility
/// of each pipeline to implement a pipeline finalizer method for its pipeline output in order to
/// update the finalized HTML for asset links and the like.
#[allow(clippy::large_enum_variant)]
pub enum TrunkLink {
    Css(Css),
    Sass(Sass),
    Icon(Icon),
    CopyFile(CopyFile),
    CopyDir(CopyDir),
    RustApp(RustApp),
    RustWorker(RustWorker),
}

impl TrunkLink {
    /// Construct a new instance.
    pub async fn from_html(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>, el: Selection<'_>, id: usize,
    ) -> Result<Self> {
        let rel = el
            .attr(ATTR_REL)
            .ok_or_else(|| anyhow!("all <link data-trunk .../> elements must have a `rel` attribute indicating the asset type"))?;
        Ok(match rel.as_ref() {
            Sass::TYPE_SASS | Sass::TYPE_SCSS => Self::Sass(Sass::new(cfg.clone(), progress, html_dir, el, id).await?),
            Icon::TYPE_ICON => Self::Icon(Icon::new(cfg.clone(), progress, html_dir, el, id).await?),
            Css::TYPE_CSS => Self::Css(Css::new(cfg.clone(), progress, html_dir, el, id).await?),
            CopyFile::TYPE_COPY_FILE => Self::CopyFile(CopyFile::new(cfg.clone(), progress, html_dir, el, id).await?),
            CopyDir::TYPE_COPY_DIR => Self::CopyDir(CopyDir::new(cfg.clone(), progress, html_dir, el, id).await?),
            RustApp::TYPE_RUST_APP => Self::RustApp(RustApp::new(cfg.clone(), progress, html_dir, ignore_chan, el, id).await?),
            RustWorker::TYPE_RUST_WORKER => Self::RustWorker(RustWorker::new(cfg.clone(), progress, html_dir, ignore_chan, el, id).await?),
            _ => bail!(
                r#"unknown <link data-trunk .../> attr value `rel="{}"`; please ensure the value is lowercase and is a supported asset type"#,
                rel.as_ref()
            ),
        })
    }

    /// Spawn the build pipeline for this asset.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        match self {
            TrunkLink::Css(inner) => inner.spawn(),
            TrunkLink::Sass(inner) => inner.spawn(),
            TrunkLink::Icon(inner) => inner.spawn(),
            TrunkLink::CopyFile(inner) => inner.spawn(),
            TrunkLink::CopyDir(inner) => inner.spawn(),
            TrunkLink::RustApp(inner) => inner.spawn(),
            TrunkLink::RustWorker(inner) => inner.spawn(),
        }
    }
}

/// The output of a `<trunk-link/>` asset pipeline.
pub enum TrunkLinkPipelineOutput {
    Css(CssOutput),
    Sass(SassOutput),
    Icon(IconOutput),
    CopyFile(CopyFileOutput),
    CopyDir(CopyDirOutput),
    RustApp(RustAppOutput),
    #[allow(dead_code)] // TODO: remove this when this pipeline type is implemented.
    RustWorker(RustWorkerOutput),
}

impl TrunkLinkPipelineOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        match self {
            TrunkLinkPipelineOutput::Css(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::Sass(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::Icon(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::CopyFile(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::CopyDir(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::RustApp(out) => out.finalize(dom).await,
            TrunkLinkPipelineOutput::RustWorker(out) => out.finalize(dom).await,
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
    pub ext: String,
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
        ensure!(path.is_file().await, "target file does not appear to exist on disk {:?}", &path);
        let file_name = match path.file_name() {
            Some(file_name) => file_name.to_owned(),
            None => bail!("asset has no file name {:?}", &path),
        };
        let file_stem = match path.file_stem() {
            Some(file_stem) => file_stem.to_owned(),
            None => bail!("asset has no file name stem {:?}", &path),
        };
        let ext = match path.extension() {
            Some(ext) => ext.to_string_lossy().to_lowercase(),
            None => bail!("asset has no file extension {:?}", &path),
        };
        Ok(Self {
            path: path.into(),
            file_name,
            file_stem,
            ext,
        })
    }

    /// Copy this asset to the target dir.
    pub async fn copy(&self, to_dir: &Path) -> Result<PathBuf> {
        let bytes = fs::read(&self.path)
            .await
            .with_context(|| format!("error reading file for copying {:?}", &self.path))?;

        let file_path = to_dir.join(&self.file_name);
        fs::write(&file_path, bytes)
            .await
            .with_context(|| format!("error copying file {:?} to {:?}", &self.path, &file_path))?;
        Ok(file_path)
    }

    /// Copy this asset to the target dir after hashing its contents & updating the filename with
    /// the hash.
    pub async fn copy_with_hash(&self, to_dir: &Path) -> Result<HashedFileOutput> {
        let bytes = fs::read(&self.path)
            .await
            .with_context(|| format!("error reading file for copying {:?}", &self.path))?;
        let hash = seahash::hash(bytes.as_ref());
        let file_name = format!("{}-{:x}.{}", &self.file_stem.to_string_lossy(), hash, &self.ext);

        let file_path = to_dir.join(&file_name);
        fs::write(&file_path, bytes)
            .await
            .with_context(|| format!("error copying file {:?} to {:?}", &self.path, &file_path))?;
        Ok(HashedFileOutput { hash, file_path, file_name })
    }
}

/// The output of a hashed file.
///
/// A file is hashed when its contents have been read, hashed, and then a new file is written with
/// the same contents, and the filename of the new file includes the hexadecimal representation of
/// the hash before the file extension, as so: `{file_stem}-{hash}.{ext}`.
pub struct HashedFileOutput {
    /// The hash of the output file.
    #[allow(dead_code)]
    hash: u64,
    /// The canonical path to the output file.
    #[allow(dead_code)]
    file_path: PathBuf,
    /// The output file's name.
    file_name: String,
}

/// Create the CSS selector for selecting a trunk link by ID.
pub(self) fn trunk_id_selector(id: usize) -> String {
    format!(r#"link[{}="{}"]"#, TRUNK_ID, id)
}
