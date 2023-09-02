//! Source HTML pipelines.

use std::borrow::Cow;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;

// use anyhow::{ensure, Context, Result};
use futures_util::stream::StreamExt;
use nipper::Document;
use tokio::fs;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use trunk_util::{AssetInput, ResultExt};

use crate::assets::{Asset, Output, RustAppConfig};
use crate::util::{Attrs, ErrorReason, Result, TRUNK_ID};

mod utils;
pub use utils::PipelineStage;

const PUBLIC_URL_MARKER_ATTR: &str = "data-trunk-public-url";
// const RELOAD_SCRIPT: &str = include_str!("../autoreload.js");

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
pub struct HtmlPipeline<C, A> {
    /// Runtime config.
    cfg: Arc<C>,
    /// The path to the source HTML document from which the output `index.html` will be built.
    target_html_path: PathBuf,
    /// The parent directory of `target_html_path`.
    target_html_dir: Arc<PathBuf>,

    asset_pipeline: A,
}

impl<C, A> HtmlPipeline<C, A>
where
    C: HtmlPipelineConfig + 'static + Sync + Send,
    A: Send + Sync + Asset + 'static,
{
    /// Create a new instance.
    pub fn new<P>(path: P, cfg: Arc<C>, asset_pipeline: A) -> Result<Self>
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
            asset_pipeline,
        })
    }

    /// Spawn this pipeline into a dedicated thread with `spawn_blocking`.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn_threaded(self) -> JoinHandle<Result<()>> {
        // NOTE WELL: this is a pattern to spawn a blocking thread, and then execute a !Send
        // future on the current thread. This is needed because nipper's internals are !Send.
        tokio::task::spawn_blocking(move || Handle::current().block_on(self.run()))
    }

    /// Run this pipeline.
    ///
    /// # Note
    ///
    /// This future is `!Send` and should be executed with a tokio `LocalSet` or `LocalPoolHandle`
    /// or `spawn_blocking`. You can call `spawn_threaded` if you don't have a Runtime that can
    /// execute `!Send` future.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(mut self) -> Result<()> {
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

        // Iterator over all `[data-trunk]` elements, assigning IDs & building pipelines.
        let assets = target_html.select(r#"[data-trunk]"#);

        for (id, asset_tag) in assets.nodes().iter().enumerate() {
            // Accumulate all attrs. The main reason we collect this as
            // raw data instead of passing around the link itself is so that we are not
            // constrained by `!Send` types.
            let attrs = asset_tag
                .attrs()
                .into_iter()
                .fold(Attrs::new(), |mut acc, attr| {
                    acc.insert(attr.name.local.as_ref().to_string(), attr.value.to_string());
                    acc
                });

            let Some(tag_name) = asset_tag.node_name().map(|m| m.to_string()) else {
                continue;
            };

            // Set the node's Trunk ID
            asset_tag.set_attr(TRUNK_ID, &id.to_string());
            let input = AssetInput {
                tag_name,
                manifest_dir: self.target_html_dir.to_path_buf(),
                id,
                attrs,
            };

            self.asset_pipeline.try_push_input(input).await?;
        }

        // // Ensure we have at most 1 Rust app pipeline to spawn.
        let rust_app_nodes = target_html
            .select(
                r#"link[data-trunk][rel="rust"][data-type="main"],
        link[data-trunk][rel="rust"]:not([data-type])"#,
            )
            .length();
        if rust_app_nodes > 1 {
            return Err(ErrorReason::RustManyMainBinary.into_error());
        }

        // Spawn all build hooks.
        let build_hooks = self.cfg.spawn_build_hooks();

        // Finalize asset pipelines.
        Self::finalize_asset_pipeline(&mut target_html, self.asset_pipeline).await?;

        // Wait for all build hooks to finish.
        if let Some(m) = build_hooks {
            m.await.reason(ErrorReason::TokioTaskFailed)??;
        }

        // Finalize HTML.
        Self::finalize_html(&self.cfg, &mut target_html);

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
    async fn finalize_asset_pipeline(target_html: &mut Document, pipeline: A) -> Result<()> {
        let mut outputs = pin!(pipeline.outputs());

        while let Some(asset_res) = outputs.next().await {
            let asset = asset_res.with_reason(|| ErrorReason::AssetFinalizeFailed)?;
            asset.finalize(target_html).await?;
        }
        Ok(())
    }

    /// Prepare the document for final output.
    fn finalize_html(cfg: &Arc<C>, target_html: &mut Document) {
        // Write public_url to base element.
        let mut base_elements =
            target_html.select(&format!("html head base[{}]", PUBLIC_URL_MARKER_ATTR));
        base_elements.remove_attr(PUBLIC_URL_MARKER_ATTR);
        base_elements.set_attr("href", RustAppConfig::public_url(cfg.as_ref()));

        if let Some(m) = cfg.append_body_str() {
            target_html.select("body").append_html(m.as_ref());
        }
    }
}
