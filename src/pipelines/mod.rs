#[cfg(test)]
mod copy_dir_test;
#[cfg(test)]
mod copy_file_test;
mod html;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use futures_util::future::ready;
use futures_util::TryFutureExt;
pub use html::HtmlPipeline;
use nipper::Document;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use trunk_pipelines::{
    CopyDir, CopyDirConfig, CopyDirOutput, CopyFile, CopyFileConfig, CopyFileOutput, Css,
    CssConfig, CssOutput, Icon, IconConfig, IconOutput, Inline, InlineOutput, Js, JsConfig,
    JsOutput, Output, Pipeline, RustApp, RustAppConfig, RustAppOutput, Sass, SassConfig,
    SassOutput, TailwindCss, TailwindCssConfig, TailwindCssOutput,
};

use crate::config::RtcBuild;

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
}

impl CopyFileConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }
}

impl RustAppConfig for RtcBuild {
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

    fn wasm_bindgen_version(&self) -> Option<&str> {
        self.tools.wasm_bindgen.as_deref()
    }

    fn wasm_opt_version(&self) -> Option<&str> {
        self.tools.wasm_opt.as_deref()
    }

    fn format_preload(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        let pattern = self.pattern_preload.as_ref()?;

        let mut params: HashMap<String, String> = match self.pattern_params {
            Some(ref x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), self.public_url.clone());
        params.insert("js".to_owned(), script_path.to_owned());
        params.insert("wasm".to_owned(), wasm_path.to_owned());

        Some(pattern_evaluate(pattern, &params))
    }

    fn format_script(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        let pattern = self.pattern_script.as_ref()?;

        let mut params: HashMap<String, String> = match self.pattern_params {
            Some(ref x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), self.public_url.clone());
        params.insert("js".to_owned(), script_path.to_owned());
        params.insert("wasm".to_owned(), wasm_path.to_owned());

        Some(pattern_evaluate(pattern, &params))
    }

    fn cargo_features(&self) -> Option<&trunk_util::Features> {
        Some(&self.cargo_features)
    }
}

pub fn pattern_evaluate(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in params.iter() {
        let pattern = format!("{{{}}}", k.as_str());
        if let Some(file_path) = v.strip_prefix('@') {
            if let Ok(contents) = std::fs::read_to_string(file_path) {
                result = str::replace(result.as_str(), &pattern, contents.as_str());
            }
        } else {
            result = str::replace(result.as_str(), &pattern, v);
        }
    }
    result
}

const ATTR_REL: &str = "rel";
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
    CopyFile(CopyFile<RtcBuild>),
    CopyDir(CopyDir<RtcBuild>),
    RustApp(RustApp<RtcBuild>),
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
                    CopyFile::<RtcBuild>::TYPE_COPY_FILE => {
                        Self::CopyFile(CopyFile::new(cfg, html_dir, attrs, id).await?)
                    }
                    CopyDir::<RtcBuild>::TYPE_COPY_DIR => {
                        Self::CopyDir(CopyDir::new(cfg, html_dir, attrs, id).await?)
                    }
                    RustApp::<RtcBuild>::TYPE_RUST_APP => {
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
            Self::CopyFile(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::CopyFile)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
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
            Self::RustApp(inner) => tokio::spawn(async move {
                inner
                    .spawn()
                    .map_ok(|m| {
                        ready(
                            m.map(TrunkAssetPipelineOutput::RustApp)
                                .map_err(anyhow::Error::from),
                        )
                    })
                    .map_err(anyhow::Error::from)
                    .try_flatten()
                    .await
            }),
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
    RustApp(RustAppOutput<RtcBuild>),
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
            TrunkAssetPipelineOutput::CopyFile(out) => {
                out.finalize(dom).await.map_err(|e| e.into())
            }
            TrunkAssetPipelineOutput::CopyDir(out) => out.finalize(dom).await.map_err(|e| e.into()),
            TrunkAssetPipelineOutput::RustApp(out) => out.finalize(dom).await.map_err(|e| e.into()),
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
