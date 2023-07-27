use std::path::PathBuf;
use std::sync::Arc;

use futures_util::future::ready;
use futures_util::TryFutureExt;
use nipper::Document;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::assets::{
    Asset, CopyDir, CopyDirConfig, CopyDirOutput, CopyFile, CopyFileConfig, CopyFileOutput, Css,
    CssConfig, CssOutput, Icon, IconConfig, IconOutput, Inline, InlineOutput, Js, JsConfig,
    JsOutput, Output, RustApp, RustAppConfig, RustAppOutput, Sass, SassConfig, SassOutput,
    TailwindCss, TailwindCssConfig, TailwindCssOutput,
};
use crate::util::{Attrs, ErrorExt, ErrorReason, Result, ResultExt, ATTR_REL};

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
pub enum TrunkAsset<C> {
    Css(Css<C>),
    Sass(Sass<C>),
    TailwindCss(TailwindCss<C>),
    Js(Js<C>),
    Icon(Icon<C>),
    Inline(Inline),
    CopyFile(CopyFile<C>),
    CopyDir(CopyDir<C>),
    RustApp(RustApp<C>),
}

impl<C> TrunkAsset<C>
where
    C: 'static
        + Sync
        + Send
        + CssConfig
        + SassConfig
        + TailwindCssConfig
        + JsConfig
        + IconConfig
        + CopyDirConfig
        + CopyFileConfig
        + RustAppConfig,
{
    /// Construct a new instance.
    pub async fn from_html(
        cfg: Arc<C>,
        html_dir: Arc<PathBuf>,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
        reference: TrunkAssetReference,
        id: usize,
    ) -> Result<Self> {
        match reference {
            TrunkAssetReference::Link(attrs) => {
                let rel = attrs.get(ATTR_REL).reason(ErrorReason::AssetRelNotFound)?;
                Ok(match rel.as_str() {
                    Sass::<C>::TYPE_SASS | Sass::<C>::TYPE_SCSS => {
                        Self::Sass(Sass::new(cfg, html_dir, attrs, id).await?)
                    }
                    Icon::<C>::TYPE_ICON => Self::Icon(Icon::new(cfg, html_dir, attrs, id).await?),
                    Inline::TYPE_INLINE => Self::Inline(Inline::new(html_dir, attrs, id).await?),
                    Css::<C>::TYPE_CSS => Self::Css(Css::new(cfg, html_dir, attrs, id).await?),
                    CopyFile::<C>::TYPE_COPY_FILE => {
                        Self::CopyFile(CopyFile::new(cfg, html_dir, attrs, id).await?)
                    }
                    CopyDir::<C>::TYPE_COPY_DIR => {
                        Self::CopyDir(CopyDir::new(cfg, html_dir, attrs, id).await?)
                    }
                    RustApp::<C>::TYPE_RUST_APP => {
                        Self::RustApp(RustApp::new(cfg, html_dir, ignore_chan, attrs, id).await?)
                    }
                    TailwindCss::<C>::TYPE_TAILWIND_CSS => {
                        Self::TailwindCss(TailwindCss::new(cfg, html_dir, attrs, id).await?)
                    }
                    _ => {
                        return Err(ErrorReason::AssetUnknownType {
                            rel_str: rel.to_owned(),
                        }
                        .into_error())
                    }
                })
            }
            TrunkAssetReference::Script(attrs) => {
                Ok(Self::Js(Js::new(cfg, html_dir, attrs, id).await?))
            }
        }
    }

    /// Spawn the build pipeline for this asset.
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput<C>>> {
        // This is a workaround, the end result should be producing a type with a builder
        // pattern that processes each Output type recursively that can finalise the when all
        // pipelines are migrated.
        match self {
            Self::Css(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::Css)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::Sass(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::Sass)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::TailwindCss(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::TailwindCss)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::Js(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::Js)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::Icon(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::Icon)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::Inline(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::Inline)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::CopyFile(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::CopyFile)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::CopyDir(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::CopyDir)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
            Self::RustApp(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| ready(m.map(TrunkAssetPipelineOutput::RustApp)))
                    .map_err(|e| e.reason(ErrorReason::TokioTaskFailed))
                    .try_flatten()
                    .await
            }),
        }
    }
}

/// The output of a `<trunk-link/>` asset pipeline.
pub enum TrunkAssetPipelineOutput<C> {
    Css(CssOutput<C>),
    Sass(SassOutput<C>),
    TailwindCss(TailwindCssOutput<C>),
    Js(JsOutput<C>),
    Icon(IconOutput<C>),
    Inline(InlineOutput),
    CopyFile(CopyFileOutput),
    CopyDir(CopyDirOutput),
    RustApp(RustAppOutput<C>),
}

impl<C> TrunkAssetPipelineOutput<C>
where
    C: 'static
        + Sync
        + Send
        + CssConfig
        + SassConfig
        + TailwindCssConfig
        + JsConfig
        + IconConfig
        + CopyDirConfig
        + CopyFileConfig
        + RustAppConfig,
{
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        match self {
            TrunkAssetPipelineOutput::Css(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::Sass(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::TailwindCss(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::Js(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::Icon(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::Inline(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::CopyFile(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::CopyDir(out) => out.finalize(dom).await,
            TrunkAssetPipelineOutput::RustApp(out) => out.finalize(dom).await,
        }
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
