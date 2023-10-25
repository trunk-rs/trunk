//! Source HTML pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use nipper::Document;
use tokio::fs;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::{RtcBuild, WsProtocol};
use crate::hooks::{spawn_hooks, wait_hooks};
use crate::pipelines::rust::RustApp;
use crate::pipelines::{
    Attrs, PipelineStage, TrunkAsset, TrunkAssetPipelineOutput, TrunkAssetReference, TRUNK_ID,
};

const PUBLIC_URL_MARKER_ATTR: &str = "data-trunk-public-url";
const RELOAD_SCRIPT: &str = include_str!("../autoreload.js");

type AssetPipelineHandles = FuturesUnordered<JoinHandle<Result<TrunkAssetPipelineOutput>>>;

/// An HTML assets build pipeline.
///
/// This build pipeline is responsible for processing the source HTML of the application, as well
/// as spawning child pipelines for any assets found in the source HTML.
pub struct HtmlPipeline {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The path to the source HTML document from which the output `index.html` will be built.
    target_html_path: PathBuf,
    /// The parent directory of `target_html_path`.
    target_html_dir: Arc<PathBuf>,
    /// An optional channel to be used to communicate ignore paths to the watcher.
    ignore_chan: Option<mpsc::Sender<PathBuf>>,
    /// Protocol used for autoreload WebSockets connection.
    pub ws_protocol: Option<WsProtocol>,
}

impl HtmlPipeline {
    /// Create a new instance.
    pub fn new(
        cfg: Arc<RtcBuild>,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
        ws_protocol: Option<WsProtocol>,
    ) -> Result<Self> {
        let target_html_path = cfg
            .target
            .canonicalize()
            .context("failed to get canonical path of target HTML file")?;
        let target_html_dir = Arc::new(
            target_html_path
                .parent()
                .context("failed to determine parent dir of target HTML file")?
                .to_owned(),
        );

        Ok(Self {
            cfg,
            target_html_path,
            target_html_dir,
            ignore_chan,
            ws_protocol,
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
        wait_hooks(spawn_hooks(self.cfg.clone(), PipelineStage::PreBuild)).await?;

        // Open the source HTML file for processing.
        let raw_html = fs::read_to_string(&self.target_html_path).await?;
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
        ensure!(
            rust_app_nodes <= 1,
            r#"only one <link data-trunk rel="rust" data-type="main" .../> may be specified"#
        );
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
        let mut pipelines: AssetPipelineHandles = FuturesUnordered::new();
        pipelines.extend(assets.into_iter().map(|asset| asset.spawn()));
        // Spawn all build hooks.
        let build_hooks = spawn_hooks(self.cfg.clone(), PipelineStage::Build);

        // Finalize asset pipelines.
        self.finalize_asset_pipelines(&mut target_html, pipelines)
            .await?;

        // Wait for all build hooks to finish.
        wait_hooks(build_hooks).await?;

        // Finalize HTML.
        self.finalize_html(&mut target_html);

        // Assemble a new output index.html file.
        let output_html = match self.cfg.release {
            true => {
                let mut minify_cfg = minify_html::Cfg::spec_compliant();
                minify_cfg.minify_css = true;
                minify_cfg.minify_js = true;
                minify_cfg.keep_closing_tags = true;
                minify_html::minify(target_html.html().as_bytes(), &minify_cfg)
            }
            false => target_html.html().as_bytes().to_vec(),
        };

        fs::write(self.cfg.staging_dist.join("index.html"), &output_html)
            .await
            .context("error writing finalized HTML output")?;

        // Spawn and wait on post-build hooks.
        wait_hooks(spawn_hooks(self.cfg.clone(), PipelineStage::PostBuild)).await?;

        Ok(())
    }

    /// Finalize asset pipelines & prep the DOM for final output.
    async fn finalize_asset_pipelines(
        &self,
        target_html: &mut Document,
        mut pipelines: AssetPipelineHandles,
    ) -> Result<()> {
        while let Some(asset_res) = pipelines.next().await {
            let asset = asset_res
                .context("failed to await asset finalization")?
                .context("error from asset pipeline")?;
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
        base_elements.set_attr("href", &self.cfg.public_url);

        dbg!(self.ws_protocol);

        // Inject the WebSocket autoloader.
        if self.cfg.inject_autoloader {
            target_html.select("body").append_html(format!(
                "<script>{}</script>",
                RELOAD_SCRIPT.replace(
                    "{{protocol}}",
                    &self
                        .ws_protocol
                        .clone()
                        .map(|p| p.to_string())
                        .unwrap_or_else(String::new)
                )
            ));
        }
    }
}
