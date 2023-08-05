//! Source HTML pipelines.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

// use anyhow::{ensure, Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use nipper::Document;
use tokio::fs;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use trunk_util::ResultExt;

use crate::assets::{
    CopyDirConfig, CopyFileConfig, CssConfig, IconConfig, JsConfig, RustApp, RustAppConfig,
    SassConfig, TailwindCssConfig,
};
use crate::util::{Attrs, ErrorReason, Result, TRUNK_ID};

mod utils;
pub use utils::PipelineStage;
use utils::{TrunkAsset, TrunkAssetPipelineOutput, TrunkAssetReference};

const PUBLIC_URL_MARKER_ATTR: &str = "data-trunk-public-url";
// const RELOAD_SCRIPT: &str = include_str!("../autoreload.js");

type AssetPipelineHandles<C> = FuturesUnordered<JoinHandle<Result<TrunkAssetPipelineOutput<C>>>>;

/// A trait that indicates a type can be used as config type for html pipeline.
pub trait HtmlPipelineConfig: RustAppConfig {
    /// Appends a string to body.
    ///
    /// This can be used to add development server script.
    fn append_body_str(&self) -> Option<Cow<'_, str>>;

    /// Spawns pre-build hooks.
    ///
    /// This function should return a join handle if pre-build hooks should be awaited before
    /// running build.
    fn spawn_pre_build_hooks(self: &Arc<Self>) -> Option<JoinHandle<Result<()>>> {
        None
    }

    /// Spawns build hooks.
    ///
    /// This function should return a join handle if build hooks should be awaited before
    /// running build.
    fn spawn_build_hooks(self: &Arc<Self>) -> Option<JoinHandle<Result<()>>> {
        None
    }

    /// Spawns post-build hooks.
    ///
    /// This function should return a join handle if post-build hooks should be awaited before
    /// running build.
    fn spawn_post_build_hooks(self: &Arc<Self>) -> Option<JoinHandle<Result<()>>> {
        None
    }
}

/// An HTML assets build pipeline.
///
/// This build pipeline is responsible for processing the source HTML of the application, as well
/// as spawning child pipelines for any assets found in the source HTML.
pub struct HtmlPipeline<C> {
    /// Runtime config.
    cfg: Arc<C>,
    /// The path to the source HTML document from which the output `index.html` will be built.
    target_html_path: PathBuf,
    /// The parent directory of `target_html_path`.
    target_html_dir: Arc<PathBuf>,
    /// An optional channel to be used to communicate ignore paths to the watcher.
    ignore_chan: Option<mpsc::Sender<PathBuf>>,
}

impl<C> HtmlPipeline<C>
where
    C: HtmlPipelineConfig
        + 'static
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
    /// Create a new instance.
    pub fn new<P>(path: P, cfg: Arc<C>, ignore_chan: Option<mpsc::Sender<PathBuf>>) -> Result<Self>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();
        let target_html_path = path
            .canonicalize()
            .with_reason(|| ErrorReason::FsNotExist { path })?;
        let target_html_dir = Arc::new(
            target_html_path
                .parent()
                .with_reason(|| ErrorReason::PathNoParent {
                    path: target_html_path.clone(),
                })?
                .to_owned(),
        );

        Ok(Self {
            cfg,
            target_html_path,
            target_html_dir,
            ignore_chan,
        })
    }

    /// Spawn a new pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self: Arc<Self>) -> JoinHandle<Result<()>> {
        // NOTE WELL: this is a pattern to spawn a blocking thread, and then execute a !Send
        // future on the current thread. This is needed because nipper's internals are !Send.
        tokio::task::spawn_blocking(move || Handle::current().block_on(self.run()))
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self: Arc<Self>) -> Result<()> {
        tracing::info!("spawning asset pipelines");

        // Spawn and wait on pre-build hooks.
        if let Some(m) = self.cfg.spawn_pre_build_hooks() {
            m.await.reason(ErrorReason::TokioTaskFailed)??;
        }

        // Open the source HTML file for processing.
        let raw_html = fs::read_to_string(&self.target_html_path)
            .await
            .with_reason(|| ErrorReason::FsReadFailed {
                path: self.target_html_path.to_path_buf(),
            })?;
        let mut target_html = Document::from(&raw_html);

        // Iterator over all `link[data-trunk]` elements, assigning IDs & building pipelines.
        let mut assets = vec![];
        let links = target_html.select(r#"link[data-trunk], script[data-trunk]"#);
        for (id, link) in links.nodes().iter().enumerate() {
            // Set the node's Trunk ID
            link.set_attr(TRUNK_ID, &id.to_string());
            let asset_ref = match link.node_name().as_deref() {
                Some("link") => {
                    // Accumulate all attrs. The main reason we collect this as
                    // raw data instead of passing around the link itself is so that we are not
                    // constrained by `!Send` types.
                    let attrs = link
                        .attrs()
                        .into_iter()
                        .fold(Attrs::new(), |mut acc, attr| {
                            acc.insert(
                                attr.name.local.as_ref().to_string(),
                                attr.value.to_string(),
                            );
                            acc
                        });

                    Some(TrunkAssetReference::Link(attrs))
                }
                Some("script") => {
                    let attrs = link
                        .attrs()
                        .into_iter()
                        .fold(Attrs::new(), |mut acc, attr| {
                            acc.insert(
                                attr.name.local.as_ref().to_string(),
                                attr.value.to_string(),
                            );
                            acc
                        });
                    Some(TrunkAssetReference::Script(attrs))
                }
                _ => None,
            };

            if let Some(asset_ref) = asset_ref {
                let asset = TrunkAsset::from_html(
                    self.cfg.clone(),
                    self.target_html_dir.clone(),
                    self.ignore_chan.clone(),
                    asset_ref,
                    id,
                )
                .await?;
                assets.push(asset);
            }
        }

        // Ensure we have a Rust app pipeline to spawn.
        let rust_app_nodes = target_html
            .select(r#"link[data-trunk][rel="rust"][data-type="main"], link[data-trunk][rel="rust"]:not([data-type])"#)
            .length();
        if rust_app_nodes > 1 {
            return Err(ErrorReason::RustManyMainBinary.into_error());
        }
        if rust_app_nodes == 0 {
            if let Ok(app) = RustApp::new_default(
                self.cfg.clone(),
                self.target_html_dir.clone(),
                self.ignore_chan.clone(),
            )
            .await
            {
                assets.push(TrunkAsset::RustApp(app));
            } else {
                tracing::warn!("no rust project found")
            };
        }

        // Spawn all asset pipelines.
        let mut pipelines: AssetPipelineHandles<C> = FuturesUnordered::new();
        pipelines.extend(assets.into_iter().map(|asset| asset.spawn()));
        // Spawn all build hooks.
        let build_hooks = self.cfg.spawn_build_hooks();

        // Finalize asset pipelines.
        self.finalize_asset_pipelines(&mut target_html, pipelines)
            .await?;

        // Wait for all build hooks to finish.
        if let Some(m) = build_hooks {
            m.await.reason(ErrorReason::TokioTaskFailed)??;
        }

        // Finalize HTML.
        self.finalize_html(&mut target_html);

        // Assemble a new output index.html file.
        let output_html = target_html.html().to_string(); // TODO: prettify this output.
        let output_path = RustAppConfig::output_dir(self.cfg.as_ref()).join("index.html");
        fs::write(&output_path, &output_html)
            .await
            .with_reason(|| ErrorReason::FsWriteFailed { path: output_path })?;

        // Spawn and wait on post-build hooks.
        if let Some(m) = self.cfg.spawn_post_build_hooks() {
            m.await.reason(ErrorReason::TokioTaskFailed)??;
        }

        Ok(())
    }

    /// Finalize asset pipelines & prep the DOM for final output.
    async fn finalize_asset_pipelines(
        &self,
        target_html: &mut Document,
        mut pipelines: AssetPipelineHandles<C>,
    ) -> Result<()> {
        while let Some(asset_res) = pipelines.next().await {
            let asset = asset_res
                .with_reason(|| ErrorReason::AssetFinalizeFailed)?
                .with_reason(|| ErrorReason::AssetFinalizeFailed)?;
            asset.finalize(target_html).await?;
        }
        Ok(())
    }

    /// Prepare the document for final output.
    fn finalize_html(&self, target_html: &mut Document) {
        // Write public_url to base element.
        let mut base_elements =
            target_html.select(&format!("html head base[{}]", PUBLIC_URL_MARKER_ATTR));
        base_elements.remove_attr(PUBLIC_URL_MARKER_ATTR);
        base_elements.set_attr("href", RustAppConfig::public_url(self.cfg.as_ref()));

        if let Some(m) = self.cfg.append_body_str() {
            target_html.select("body").append_html(m.as_ref());
        }
    }
}
