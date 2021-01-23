//! Source HTML pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use async_std::fs;
use async_std::path::Path;
use async_std::task::{spawn_local, JoinHandle};
use futures::channel::mpsc::Sender;
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use nipper::Document;

use crate::common::{copy_dir_recursive, remove_dir_all};
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
        self.prepare_staging_dist().await.context("error preparing build environment")?;

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

        self.apply_dist().await.context("error applying built distribution")?;
        Ok(())
    }

    /// Moves the contents of dist/.current into dist, signifying the application
    /// of a successful build. Also removes dist/.current afterwards.
    async fn apply_dist(self: Arc<Self>) -> Result<()> {
        let final_dist = self.cfg.final_dist.clone();
        let staging_dist = self.cfg.staging_dist.clone();
        self.progress.clone().set_message("applying new distribution");

        // build succeeded, so delete everything in `dist`,
        // copy everything from `dist/.current` to `dist`, and
        // then delete `dist/.current`
        let mut entries = fs::read_dir(&final_dist).await.context("error reading dist dir")?;
        while let Some(entry) = entries.next().await {
            let entry = entry.context("error reading contents of dist dir")?;
            if entry.file_name() == ".current" {
                continue;
            }

            let file_type = entry.file_type().await.context("error reading metadata of file in dist dir")?;

            if file_type.is_dir() {
                remove_dir_all(entry.path().into()).await.context("error cleaning dist")?;
            } else if file_type.is_symlink() || file_type.is_file() {
                fs::remove_file(entry.path()).await.context("error cleaning dist")?;
            }
        }

        copy_dir_recursive(staging_dist.to_path_buf(), final_dist.to_path_buf())
            .await
            .context("error copying dist/.current dir to dist dir")?;

        remove_dir_all(staging_dist).await.context("error deleting dist/.current")?;

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

    /// Creates a "holding area" (dist/.current) for storing intermediate build results
    async fn prepare_staging_dist(&self) -> Result<()> {
        // Prepare holding area in which we will assemble the latest build
        let staging_dist: &Path = self.cfg.staging_dist.as_path().into();

        if staging_dist.exists().await {
            // Clean holding area, if applicable
            let mut entries = fs::read_dir(staging_dist).await.context("error reading dist/.current dir")?;
            while let Some(entry) = entries.next().await {
                let entry = entry.context("error reading contents of dist/.current dir")?;
                let file_type = entry.file_type().await.context("error reading metadata of file in dist/.current dir")?;

                if file_type.is_dir() {
                    fs::remove_dir_all(entry.path()).await.context("Cleaning dist/.current failed")?;
                } else if file_type.is_symlink() || file_type.is_file() {
                    fs::remove_file(entry.path()).await.context("Cleaning dist/.current failed")?;
                }
            }
        } else {
            fs::create_dir_all(staging_dist).await.context("error creating dist/.current dir")?;
        }

        Ok(())
    }
}
