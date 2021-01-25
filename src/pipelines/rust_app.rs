//! Rust application pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use async_process::{Command, Stdio};
use futures::channel::mpsc::Sender;
use indicatif::ProgressBar;
use nipper::{Document, Selection};
use tokio::fs;
use tokio::task::{spawn, JoinHandle};

use super::TrunkLinkPipelineOutput;
use super::{ATTR_HREF, SNIPPETS_DIR};
use crate::common::copy_dir_recursive;
use crate::config::{CargoMetadata, RtcBuild};

/// A Rust application pipeline.
pub struct RustApp {
    /// The ID of this pipeline's source HTML element.
    id: Option<usize>,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate paths to ignore back to the watcher.
    ignore_chan: Option<Sender<PathBuf>>,
    /// An optional binary name which will cause cargo & wasm-bindgen to process only the target
    /// binary.
    bin: Option<String>,
}

impl RustApp {
    pub const TYPE_RUST_APP: &'static str = "rust";

    pub async fn new(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>, el: Selection<'_>, id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let manifest_href = el
            .attr(ATTR_HREF)
            .map(|tendril| {
                let mut path = PathBuf::new();
                path.extend(tendril.as_ref().split('/'));
                if !path.is_absolute() {
                    path = html_dir.join(path);
                }
                if !path.ends_with("Cargo.toml") {
                    path = path.join("Cargo.toml");
                }
                path
            })
            .unwrap_or_else(|| html_dir.join("Cargo.toml"));
        let bin = el.attr("data-bin").map(|val| val.to_string());
        let manifest = CargoMetadata::new(&manifest_href).await?;
        let id = Some(id);

        Ok(Self {
            id,
            cfg,
            progress,
            manifest,
            ignore_chan,
            bin,
        })
    }

    pub async fn new_default(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>,
    ) -> Result<Self> {
        let path = html_dir.join("Cargo.toml");
        let manifest = CargoMetadata::new(&path).await?;
        Ok(Self {
            id: None,
            cfg,
            progress,
            manifest,
            ignore_chan,
            bin: None,
        })
    }

    /// Spawn a new pipeline.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.build())
    }

    async fn build(mut self) -> Result<TrunkLinkPipelineOutput> {
        let (wasm, hashed_name) = self.cargo_build().await?;
        let output = self.wasm_bindgen_build(wasm, hashed_name).await?;
        Ok(TrunkLinkPipelineOutput::RustApp(output))
    }

    async fn cargo_build(&mut self) -> Result<(PathBuf, String)> {
        self.progress.set_message(&format!("building {}", &self.manifest.package.name));
        if let Some(chan) = &mut self.ignore_chan {
            let _ = chan.try_send(self.manifest.metadata.target_directory.clone());
        }

        // Spawn the cargo build process.
        let mut args = vec![
            "build",
            "--target=wasm32-unknown-unknown",
            "--manifest-path",
            &self.manifest.manifest_path,
        ];
        if self.cfg.release {
            args.push("--release");
        }
        if let Some(bin) = &self.bin {
            args.push("--bin");
            args.push(bin);
        }
        let build_output = Command::new("cargo")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning cargo build")?
            .output()
            .await
            .context("error during cargo build execution")?;
        ensure!(
            build_output.status.success(),
            "bad status returned from cargo build: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );

        // Perform a final cargo invocation on success to get artifact names.
        self.progress.set_message("fetching artifacts");
        args.push("--message-format=json");
        let artifacts_out = Command::new("cargo")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning cargo build artifacts task")?
            .output()
            .await
            .context("error getting cargo build artifacts info")?;
        ensure!(
            artifacts_out.status.success(),
            "bad status returned from cargo artifacts request: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );

        // Stream over cargo messages to find the artifacts we are interested in.
        let reader = std::io::BufReader::new(artifacts_out.stdout.as_slice());
        let artifact = cargo_metadata::Message::parse_stream(reader)
            .filter_map(|msg| if let Ok(msg) = msg { Some(msg) } else { None })
            .fold(Ok(None), |acc, msg| match msg {
                cargo_metadata::Message::CompilerArtifact(art) if art.package_id == self.manifest.package.id => Ok(Some(art)),
                cargo_metadata::Message::BuildFinished(finished) if !finished.success => Err(anyhow!("error while fetching cargo artifact info")),
                _ => acc,
            })?
            .ok_or_else(|| anyhow!("cargo artifacts not found for target crate"))?;

        // Get a handle to the WASM output file.
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .ok_or_else(|| anyhow!("could not find WASM output after cargo build"))?;

        // Hash the built wasm app, then use that as the out-name param.
        self.progress.set_message("processing WASM");
        let wasm_bytes = fs::read(&wasm).await.context("error reading wasm file for hash generation")?;
        let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes));
        Ok((wasm, hashed_name))
    }

    async fn wasm_bindgen_build(&self, wasm: PathBuf, hashed_name: String) -> Result<RustAppOutput> {
        self.progress.set_message("calling wasm-bindgen");

        // Ensure our output dir is in place.
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let bindgen_out = self.manifest.metadata.target_directory.join("wasm-bindgen").join(mode_segment);
        fs::create_dir_all(bindgen_out.as_path())
            .await
            .context("error creating wasm-bindgen output dir")?;

        // Build up args for calling wasm-bindgen.
        let arg_out_path = format!("--out-dir={}", bindgen_out.display());
        let arg_out_name = format!("--out-name={}", &hashed_name);
        let target_wasm = wasm.to_string_lossy().to_string();
        let args = vec!["--target=web", &arg_out_path, &arg_out_name, "--no-typescript", &target_wasm];

        // Invoke wasm-bindgen.
        let build_output = Command::new("wasm-bindgen")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning wasm-bindgen call")?
            .output()
            .await
            .context("error during wasm-bindgen call")?;
        ensure!(
            build_output.status.success(),
            "wasm-bindgen call returned a bad status {}",
            String::from_utf8_lossy(&build_output.stderr),
        );

        // Copy the generated WASM & JS loader to the dist dir.
        self.progress.set_message("copying generated artifacts");
        let hashed_js_name = format!("{}.js", &hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &hashed_name);
        let js_loader_path = bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = self.cfg.dist.join(&hashed_js_name);
        let wasm_path = bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = self.cfg.dist.join(&hashed_wasm_name);
        fs::copy(js_loader_path, js_loader_path_dist)
            .await
            .context("error copying JS loader file to dist dir")?;
        fs::copy(wasm_path, wasm_path_dist).await.context("error copying wasm file to dist dir")?;

        // Check for any snippets, and copy them over.
        let snippets_dir = bindgen_out.join(SNIPPETS_DIR);
        // tokio#3373 would provide a better API for checking if a path exists
        if fs::metadata(&snippets_dir).await.is_ok() {
            copy_dir_recursive(bindgen_out.join(SNIPPETS_DIR), self.cfg.dist.join(SNIPPETS_DIR))
                .await
                .context("error copying snippets dir")?;
        }

        Ok(RustAppOutput {
            id: self.id,
            cfg: self.cfg.clone(),
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
        })
    }
}

/// The output of a cargo build pipeline.
pub struct RustAppOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: Option<usize>,
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}

impl RustAppOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let script = format!(
            r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
            base = self.cfg.public_url,
            js = &self.js_output,
            wasm = &self.wasm_output,
        );
        match self.id {
            Some(id) => dom.select(&super::trunk_id_selector(id)).replace_with_html(script),
            None => dom.select("html head").append_html(script),
        }
        Ok(())
    }
}
