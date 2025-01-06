//! Source HTML pipelines.

use crate::{
    common::{
        html_rewrite::{Document, DocumentOptions},
        nonce_attr,
    },
    config::{rt::RtcBuild, types::WsProtocol},
    hooks::{spawn_hooks, wait_hooks},
    pipelines::{
        rust::RustApp, Attrs, PipelineStage, TrunkAsset, TrunkAssetPipelineOutput,
        TrunkAssetReference, TRUNK_ID,
    },
    processing::minify::minify_html,
};
use anyhow::{ensure, Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::{path::PathBuf, sync::Arc};
use tokio::{
    fs,
    sync::mpsc,
    task::{JoinError, JoinHandle},
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
    ws_protocol: Option<WsProtocol>,
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
        tokio::spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self: Arc<Self>) -> Result<()> {
        tracing::debug!("spawning asset pipelines");

        // Spawn and wait on pre-build hooks.
        wait_hooks(spawn_hooks(self.cfg.clone(), PipelineStage::PreBuild)).await?;

        // Open the source HTML file for processing.
        let raw_html = fs::read(&self.target_html_path).await?;
        let mut target_html = Document::new(
            raw_html,
            DocumentOptions {
                allow_self_closing_script: self.cfg.allow_self_closing_script,
            },
        )?;
        let mut partial_assets = vec![];

        // Since the `lol_html` doesn't provide an iterator for elements, we must use our own id.
        let mut id = 0;

        // Setting, and removing attributes could be implemented as a method for `Document`.
        // However, each selection performed causes a full rewrite of the Html content.
        // Doing things this way is likely to be better performing for larger files.
        //
        // This is the first parsing of the HTML meaning it is pretty likely to receive
        // invalid HTML at this stage.
        target_html.select_mut(r#"link[data-trunk], script[data-trunk]"#, |el| {
            'l: {
                el.set_attribute(TRUNK_ID, &id.to_string())?;

                // Both are function pointers, no need to branch out.
                let asset_constructor = match el.tag_name().as_str() {
                    "link" => TrunkAssetReference::Link,
                    "script" => TrunkAssetReference::Script,
                    // Just an early break since we won't do anything else.
                    _ => break 'l,
                };

                // Accumulate all attrs. The main reason we collect this as
                // raw data instead of passing around the link itself, is the lifetime
                // requirements of elements used in `lol_html::html_content::HtmlRewriter`.
                let attrs = el.attributes().iter().fold(Attrs::new(), |mut acc, attr| {
                    acc.insert(attr.name(), attr.value());
                    acc
                });

                let asset = TrunkAsset::from_html(
                    self.cfg.clone(),
                    self.target_html_dir.clone(),
                    self.ignore_chan.clone(),
                    asset_constructor(attrs),
                    id,
                );

                partial_assets.push(asset);
            }
            id += 1;
            Ok(())
        })?;

        let mut assets: Vec<TrunkAsset> = futures_util::future::join_all(partial_assets)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        // Ensure we have a Rust app pipeline to spawn.
        let rust_app_nodes = target_html
            .len(r#"link[data-trunk][rel="rust"][data-type="main"], link[data-trunk][rel="rust"]:not([data-type])"#)?;
        ensure!(
            rust_app_nodes <= 1,
            r#"only one <link data-trunk rel="rust" data-type="main" .../> may be specified"#
        );
        if rust_app_nodes == 0 {
            if let Some(app) = RustApp::new_default(
                self.cfg.clone(),
                self.target_html_dir.clone(),
                self.ignore_chan.clone(),
            )
            .await?
            {
                assets.push(TrunkAsset::RustApp(app));
            } else {
                tracing::warn!("no rust project found")
            };
        }

        // Spawn all asset pipelines.
        let mut pipelines: AssetPipelineHandles = FuturesUnordered::new();
        pipelines.extend(assets.into_iter().map(TrunkAsset::spawn));
        // Spawn all build hooks.
        let build_hooks = spawn_hooks(self.cfg.clone(), PipelineStage::Build);

        // Finalize asset pipelines.
        self.finalize_asset_pipelines(&mut target_html, pipelines)
            .await?;

        // Wait for all build hooks to finish.
        wait_hooks(build_hooks).await?;

        // Finalize HTML.
        self.finalize_html(&mut target_html)?;

        // Assemble a new output index.html file.
        let output_html = match self.cfg.should_minify() {
            true => minify_html(target_html.into_inner().as_slice()),
            false => target_html.into_inner(),
        };

        fs::write(
            self.cfg.staging_dist.join(&self.cfg.html_output_filename),
            &output_html,
        )
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
        let mut errors = Vec::new();

        /// finalize an asset pipeline with a single result
        async fn finalize(
            asset_res: std::result::Result<Result<TrunkAssetPipelineOutput>, JoinError>,
            target_html: &mut Document,
        ) -> Result<()> {
            let asset = asset_res
                .context("failed to await asset pipeline")?
                .context("error from asset pipeline")?;

            asset
                .finalize(target_html)
                .await
                .context("failed to finalize asset pipeline")?;

            Ok(())
        }

        // pull all results and store their errors
        while let Some(asset_res) = pipelines.next().await {
            if let Err(err) = finalize(asset_res, target_html).await {
                // store the error, but don't return, so that we can still await all others
                errors.push(err);
            }
        }

        // now check for errors
        if let Some(first) = errors.pop() {
            // if we have some, fail with the first
            return Err(first.context(format!(
                "HTML build pipeline failed ({} errors), showing first",
                errors.len() + 1
            )));
        }

        // return only once all pipeline steps have completed, so that we don't start a new build
        // while previous pipelines are still running

        Ok(())
    }

    /// Prepare the document for final output.
    fn finalize_html(&self, target_html: &mut Document) -> Result<()> {
        // Write public_url to base element.
        target_html.select_mut(
            &format!("html head base[{}]", PUBLIC_URL_MARKER_ATTR),
            |el| {
                el.remove_attribute(PUBLIC_URL_MARKER_ATTR);
                el.set_attribute("href", &self.cfg.public_url)?;
                Ok(())
            },
        )?;

        // Inject the WebSocket autoloader.
        if self.cfg.inject_autoloader {
            target_html.append_html(
                "body",
                &format!(
                    "<script{}>{}</script>",
                    nonce_attr(&self.cfg.create_nonce),
                    RELOAD_SCRIPT.replace(
                        "{{__TRUNK_WS_PROTOCOL__}}",
                        &self.ws_protocol.map(|p| p.to_string()).unwrap_or_default()
                    )
                ),
            )?;
        }

        Ok(())
    }
}
