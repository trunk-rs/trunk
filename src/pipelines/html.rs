//! Source HTML pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_std::fs;
use async_std::task::{spawn, spawn_blocking, spawn_local, JoinHandle};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use nipper::Document;

use crate::config::RtcBuild;
use crate::pipelines::assets::{AssetFile, AssetPipelineOutput, AssetType};
use crate::pipelines::wasmbg::WasmBindgenOutput;

const HREF_ATTR: &str = "href";
const PUBLIC_URL_MARKER_ATTR: &str = "data-trunk-public-url";
const TRUNK_ID: &str = "__trunk-id";

type AssetPipelineHandles = FuturesUnordered<JoinHandle<Result<AssetPipelineOutput>>>;

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
    target_html_dir: PathBuf,
}

impl HtmlPipeline {
    /// Create a new instance.
    pub fn new(cfg: Arc<RtcBuild>, progress: ProgressBar) -> Result<Self> {
        let target_html_path = cfg.target.canonicalize().context("failed to get canonical path of target HTML file")?;
        let target_html_dir = target_html_path
            .parent()
            .ok_or_else(|| anyhow!("failed to determine parent dir of target HTML file"))?
            .to_owned();

        Ok(Self {
            cfg,
            progress,
            target_html_path,
            target_html_dir,
        })
    }

    /// Spawn a new pipeline.
    pub fn spawn(self: Arc<Self>, wasmbg: JoinHandle<Result<WasmBindgenOutput>>) -> JoinHandle<Result<()>> {
        spawn_local(self.build(wasmbg))
    }

    /// Perform the build routine of this pipeline.
    async fn build(self: Arc<Self>, wasmbg: JoinHandle<Result<WasmBindgenOutput>>) -> Result<()> {
        self.progress.set_message("spawning asset pipelines");

        // TODO: this needs to be refactored a bit to work in a more generic fashion.
        // See #50, #46, #28, #3

        // Open the source HTML file for processing.
        let raw_html = fs::read_to_string(&self.target_html_path).await?;
        let mut target_html = Document::from(&raw_html);

        // Spawn pipelines for any links in the HTML head which need to be processed.
        let asset_links = target_html
            .select(r#"html head link"#)
            .iter()
            .filter_map(|node| {
                // Be sure our link has an href to process, else skip.
                let href = match node.attr(HREF_ATTR) {
                    Some(href) => href,
                    None => return None,
                };
                // Skip if there is an obvious protocol segment. This filters out `file://`
                // patterns as well. Just don't use the `file://` pattern, it is not needed.
                if href.contains("://") {
                    return None;
                }
                Some((node, href))
            })
            .enumerate();

        // Update the DOM for each extracted asset as long as it is a valid FS path.
        let mut assets = vec![];
        for (idx, (mut node, href)) in asset_links {
            // Handle given paths in a platform agnostic manner.
            let mut path = PathBuf::new();
            path.extend(href.as_ref().split('/')); // This handles conversion to Windows paths when needed.
            if !path.is_absolute() {
                path = self.target_html_dir.join(path);
            }

            // Take the path to referenced resource, if it is a valid asset, then we continue.
            let rel = node.attr_or("rel", "").to_string().to_lowercase();
            let id = format!("link-{}", idx);
            let asset = match AssetFile::new(path, AssetType::Link { rel }, id, &self.progress).await {
                Ok(asset) => asset,
                Err(_) => continue,
            };
            // Update the DOM with an ID for async processing.
            node.set_attr(TRUNK_ID, &asset.id);
            assets.push(asset);
        }

        // Route assets over to the appropriate pipeline handler.
        let pipelines: AssetPipelineHandles = FuturesUnordered::new();
        for asset in assets {
            if let Some(handle) = self.spawn_asset_bundle(asset) {
                pipelines.push(handle);
            }
        }

        // Finalize asset pipelines.
        self.progress.set_message("awaiting asset pipelines");
        self.finalize_asset_pipelines(&mut target_html, pipelines).await;
        self.progress.set_message("awaiting wasm-bindgen pipeline");
        let wasmbg_out = wasmbg.await?;
        self.insert_wasm_module(&wasmbg_out, &mut target_html);
        self.finalize_html(&mut target_html);

        // Assemble a new output index.html file.
        let output_html = target_html.html(); // TODO: prettify this output.
        fs::write(format!("{}/index.html", self.cfg.dist.display()), output_html.as_bytes())
            .await
            .context("error writing finalized HTML output")?;
        Ok(())
    }

    /// Finalize asset pipelines & prep the DOM for final output.
    async fn finalize_asset_pipelines(&self, target_html: &mut Document, mut pipelines: AssetPipelineHandles) {
        while let Some(asset_res) = pipelines.next().await {
            // Unpack the asset pipeline result.
            let asset = match asset_res {
                Ok(asset) => asset,
                Err(err) => {
                    self.progress.println(format!("{}", err));
                    continue;
                }
            };
            // Update the DOM based on asset output.
            let mut node = target_html.select(&format!("[{}={}]", TRUNK_ID, &asset.id));
            if asset.remove {
                node.remove();
            } else {
                node.remove_attr(TRUNK_ID);
                node.remove_attr(HREF_ATTR);
                node.set_attr(HREF_ATTR, &format!("{}{}", &self.cfg.public_url, &asset.file_name));
            }
        }
        // Remove any additional trunk IDs from the DOM.
        target_html.select(&format!("[{}]", TRUNK_ID)).remove_attr(TRUNK_ID);
    }

    /// Prepare the document for final output.
    fn finalize_html(&self, target_html: &mut Document) {
        // write public_url to base elements
        let mut base_elements = target_html.select(&format!("html head base[{}]", PUBLIC_URL_MARKER_ATTR));
        base_elements.remove_attr(PUBLIC_URL_MARKER_ATTR);
        base_elements.set_attr("href", &self.cfg.public_url);
    }

    /// Insert the finalized WASM into the output HTML.
    fn insert_wasm_module(&self, wasm: &WasmBindgenOutput, target_html: &mut Document) {
        let script = format!(
            r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
            base = self.cfg.public_url,
            js = &wasm.js_output,
            wasm = &wasm.wasm_output,
        );
        target_html.select("head").append_html(script);
    }

    /// Spawn an build pipeline for the given asset based on its file extension.
    fn spawn_asset_bundle(&self, asset: AssetFile) -> Option<JoinHandle<Result<AssetPipelineOutput>>> {
        match &asset.atype {
            AssetType::Link { rel } => match rel.as_ref() {
                "stylesheet" => match asset.ext.as_ref() {
                    "scss" | "sass" => Some(self.spawn_sass_pipeline(asset)),
                    "css" => Some(self.spawn_copy_pipeline(asset, true, false)),
                    _ => Some(self.spawn_copy_pipeline(asset, false, false)),
                },
                "icon" => Some(self.spawn_copy_pipeline(asset, true, false)),
                "trunk-dist" => Some(self.spawn_copy_pipeline(asset, false, true)),
                _ => None,
            },
        }
    }

    /// Spawn a concurrent build pipeline for a SASS/SCSS asset.
    fn spawn_sass_pipeline(&self, asset: AssetFile) -> JoinHandle<Result<AssetPipelineOutput>> {
        let (dist, release, progress) = (self.cfg.dist.clone(), self.cfg.release, self.progress.clone());
        spawn(async move {
            // Compile the target SASS/SCSS file.
            let path_str = asset.path.to_string_lossy().to_string();
            let mut opts = sass_rs::Options::default();
            if release {
                opts.output_style = sass_rs::OutputStyle::Compressed;
            }
            let css = spawn_blocking(move || match sass_rs::compile_file(&path_str, opts) {
                Ok(css) => Ok(css),
                Err(err) => {
                    progress.println(err);
                    Err(anyhow!("error compiling sass for {}", &path_str))
                }
            })
            .await?;
            // Hash the contents to generate a file name, and then write the contents to the dist dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = asset.file_stem.to_string_lossy();
            let out_file_name = format!("{}-{:x}.css", file_name, hash);
            let out_file = dist.join(&out_file_name);
            fs::write(out_file, css).await.context("error writing SASS pipeline output")?;
            Ok(AssetPipelineOutput {
                id: asset.id,
                file_name: out_file_name,
                remove: false,
            })
        })
    }

    /// Spawn a concurrent build pipeline which simply copies the source to the destination, unchanged.
    fn spawn_copy_pipeline(&self, asset: AssetFile, hash: bool, remove: bool) -> JoinHandle<Result<AssetPipelineOutput>> {
        let dist = self.cfg.dist.clone();
        spawn(async move {
            let bytes = fs::read(&asset.path)
                .await
                .with_context(|| format!("error reading file for hashing in copy pipeline {:?}", &asset.path))?;
            let new_file_name = if hash {
                let hash = seahash::hash(bytes.as_ref());
                let orig_file_name = asset.file_stem.to_string_lossy();
                format!("{}-{:x}.{}", orig_file_name, hash, &asset.ext)
            } else {
                asset.file_name.to_string_lossy().to_string()
            };

            let out_file_name = dist.join(&new_file_name);
            fs::write(out_file_name, bytes)
                .await
                .with_context(|| format!("error copying file in copy pipeline {:?}", &asset.path))?;

            Ok(AssetPipelineOutput {
                id: asset.id,
                file_name: new_file_name,
                remove,
            })
        })
    }
}
