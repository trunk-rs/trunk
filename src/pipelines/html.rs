//! Source HTML pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use async_std::fs;
use async_std::task::{spawn_local, JoinHandle};
use futures::channel::mpsc::Sender;
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use nipper::Document;

use crate::config::RtcBuild;
use crate::pipelines::rust_app::RustApp;
use crate::pipelines::{TrunkLink, TrunkLinkPipelineOutput, TRUNK_ID};

const PUBLIC_URL_MARKER_ATTR: &str = "data-trunk-public-url";

type AssetPipelineHandles = FuturesUnordered<JoinHandle<Result<TrunkLinkPipelineOutput>>>;

/// An HTML assets build pipeline.
///
/// This build pipeline is responsible for processing the source HTML of the application, as well
/// as spawning child pipelines for any assets found in the source HTML.
pub struct HtmlPipeline {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
    /// The path to the source HTML document from which the output `index.html` will be built.
    target_html_path: PathBuf,
    /// The parent directory of `target_html_path`.
    target_html_dir: Arc<PathBuf>,
    /// An optional channel to be used to communicate ignore paths to the watcher.
    ignore_chan: Option<Sender<PathBuf>>,
}

impl HtmlPipeline {
    /// Create a new instance.
    pub fn new(cfg: Arc<RtcBuild>, progress: ProgressBar, ignore_chan: Option<Sender<PathBuf>>) -> Result<Self> {
        let target_html_path = cfg.target.canonicalize().context("failed to get canonical path of target HTML file")?;
        let target_html_dir = Arc::new(
            target_html_path
                .parent()
                .ok_or_else(|| anyhow!("failed to determine parent dir of target HTML file"))?
                .to_owned(),
        );

        Ok(Self {
            cfg,
            progress,
            target_html_path,
            target_html_dir,
            ignore_chan,
        })
    }

    /// Spawn a new pipeline.
    pub fn spawn(self: Arc<Self>) -> JoinHandle<Result<()>> {
        spawn_local(self.build())
    }

    /// Perform the build routine of this pipeline.
    async fn build(self: Arc<Self>) -> Result<()> {
        self.progress.clone().set_message("spawning asset pipelines");

        // Open the source HTML file for processing.
        let raw_html = fs::read_to_string(&self.target_html_path).await?;
        let mut target_html = Document::from(&raw_html);

        // Iterator over all `link[data-trunk]` elements, assigning IDs & building pipelines.
        let mut assets = vec![];
        for (id, mut link) in target_html.select(r#"link[data-trunk]"#).iter().enumerate() {
            link.set_attr(TRUNK_ID, &id.to_string());
            let asset = TrunkLink::from_html(
                self.cfg.clone(),
                self.progress.clone(),
                self.target_html_dir.clone(),
                self.ignore_chan.clone(),
                link,
                id,
            )
            .await?;
            assets.push(asset);
        }

        // Ensure we have a Rust app pipeline to spawn.
        let rust_app_nodes = target_html.select(r#"link[data-trunk][rel="rust"]"#).length();
        ensure!(rust_app_nodes <= 1, r#"only one <link data-trunk rel="rust" .../> link may be specified"#);
        if rust_app_nodes == 0 {
            let app = RustApp::new_default(
                self.cfg.clone(),
                self.progress.clone(),
                self.target_html_dir.clone(),
                self.ignore_chan.clone(),
            )
            .await?;
            assets.push(TrunkLink::RustApp(app));
        }

        // Spawn all asset pipelines.
        let mut pipelines: AssetPipelineHandles = FuturesUnordered::new();
        pipelines.extend(assets.into_iter().map(|asset| asset.spawn()));

        // Finalize asset pipelines.
        self.finalize_asset_pipelines(&mut target_html, pipelines).await?;
        self.finalize_html(&mut target_html);

        // Assemble a new output index.html file.
        let output_html = target_html.html(); // TODO: prettify this output.
        fs::write(self.cfg.staging_dist.join("index.html"), output_html.as_bytes())
            .await
            .context("error writing finalized HTML output")?;

        Ok(())
    }

    /// Finalize asset pipelines & prep the DOM for final output.
    async fn finalize_asset_pipelines(&self, target_html: &mut Document, mut pipelines: AssetPipelineHandles) -> Result<()> {
        while let Some(asset_res) = pipelines.next().await {
            let asset = asset_res?;
            asset.finalize(target_html).await?;
        }
        Ok(())
    }

    /// Prepare the document for final output.
    fn finalize_html(&self, target_html: &mut Document) {
        // Write public_url to base element.
        let mut base_elements = target_html.select(&format!("html head base[{}]", PUBLIC_URL_MARKER_ATTR));
        base_elements.remove_attr(PUBLIC_URL_MARKER_ATTR);
        base_elements.set_attr("href", &self.cfg.public_url);
    }
}
