//! Rust web worker pipeline.

#![allow(dead_code, unused_variables)] // TODO: remove this when this pipeline type is implemented.

use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use async_process::{Command, Stdio};
use async_std::fs;
use async_std::task::{spawn, JoinHandle};
use futures::channel::mpsc::Sender;
use nipper::Document;

use super::{LinkAttrs, TrunkLinkPipelineOutput};
use super::{ATTR_HREF, SNIPPETS_DIR};
use crate::common::{copy_dir_recursive, path_exists, run_command};
use crate::config::{CargoMetadata, RtcBuild};
use crate::wasm_opt::{wasm_opt_build, WasmOptLevel};

/// A Rust web worker pipeline.
pub struct RustWorker {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate ignore paths to the watcher.
    ignore_chan: Option<Sender<PathBuf>>,
    /// An optional binary name which will cause cargo & wasm-bindgen to process only the target
    /// binary.
    bin: Option<String>,
    /// An option to instruct wasm-bindgen to preserve debug info in the final WASM output, even
    /// for `--release` mode.
    keep_debug: bool,
    /// An option to instruct wasm-bindgen to not demangle Rust symbol names.
    no_demangle: bool,
    /// An optional optimization setting that enables wasm-opt. Can be nothing, `0` (default), `1`,
    /// `2`, `3`, `4`, `s or `z`. Using `0` disables wasm-opt completely.
    wasm_opt: WasmOptLevel,
}

impl RustWorker {
    pub const TYPE_RUST_WORKER: &'static str = "rust-worker";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let manifest_href = attrs
            .get(ATTR_HREF)
            .map(|attr| {
                let mut path = PathBuf::new();
                path.extend(attr.split('/'));
                if !path.is_absolute() {
                    path = html_dir.join(path);
                }
                if !path.ends_with("Cargo.toml") {
                    path = path.join("Cargo.toml");
                }
                path
            })
            .unwrap_or_else(|| html_dir.join("Cargo.toml"));
        let bin = attrs.get("data-bin").map(|val| val.to_string());
        let keep_debug = attrs.contains_key("data-keep-debug");
        let no_demangle = attrs.contains_key("data-no-demangle");
        let wasm_opt = attrs.get("data-wasm-opt").map(|val| val.parse()).transpose()?.unwrap_or_default();
        let manifest = CargoMetadata::new(&manifest_href).await?;

        Ok(Self {
            id,
            cfg,
            manifest,
            ignore_chan,
            bin,
            keep_debug,
            no_demangle,
            wasm_opt,
        })
    }

    /// Spawn a new pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.build())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn build(mut self) -> Result<TrunkLinkPipelineOutput> {
        let (wasm, hashed_name) = self.cargo_build().await?;
        let output = self.wasm_bindgen_build(wasm.as_ref(), &hashed_name).await?;
        wasm_opt_build(self.cfg, self.manifest, self.wasm_opt, &output.wasm_output).await?;
        Ok(TrunkLinkPipelineOutput::RustWorker(output))
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn cargo_build(&mut self) -> Result<(PathBuf, String)> {
        tracing::info!("building {}", &self.manifest.package.name);

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
        let build_res = run_command("cargo", &args).await.context("error during cargo build execution");

        // Send cargo's target dir over to the watcher to be ignored. We must do this before
        // checking for errors, otherwise the dir will never be ignored. If we attempt to do
        // this pre-build, the canonicalization will fail and will not be ignored.
        if let Some(chan) = &mut self.ignore_chan {
            let _ = chan.try_send(self.manifest.metadata.target_directory.clone());
        }

        // Now propagate any errors which came from the cargo build.
        let _ = build_res?;

        // Perform a final cargo invocation on success to get artifact names.
        tracing::info!("fetching cargo artifacts");
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
        if !artifacts_out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&artifacts_out.stderr));
            bail!("bad status returned from cargo artifacts request");
        }

        // Stream over cargo messages to find the artifacts we are interested in.
        let reader = std::io::BufReader::new(artifacts_out.stdout.as_slice());
        let artifact = cargo_metadata::Message::parse_stream(reader)
            .filter_map(|msg| if let Ok(msg) = msg { Some(msg) } else { None })
            .fold(Ok(None), |acc, msg| match msg {
                cargo_metadata::Message::CompilerArtifact(art) if art.package_id == self.manifest.package.id => Ok(Some(art)),
                cargo_metadata::Message::BuildFinished(finished) if !finished.success => Err(anyhow!("error while fetching cargo artifact info")),
                _ => acc,
            })?
            .context("cargo artifacts not found for target crate")?;

        // Get a handle to the WASM output file.
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .context("could not find WASM output after cargo build")?;

        // Hash the built wasm app, then use that as the out-name param.
        tracing::info!("processing WASM");
        let wasm_bytes = fs::read(&wasm).await.context("error reading wasm file for hash generation")?;
        let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes));
        Ok((wasm, hashed_name))
    }

    #[tracing::instrument(level = "trace", skip(self, wasm, hashed_name))]
    async fn wasm_bindgen_build(&self, wasm: &Path, hashed_name: &str) -> Result<RustWorkerOutput> {
        tracing::info!("calling wasm-bindgen");

        // Ensure our output dir is in place.
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let bindgen_out = self.manifest.metadata.target_directory.join("wasm-bindgen").join(mode_segment);
        fs::create_dir_all(bindgen_out.as_path())
            .await
            .context("error creating wasm-bindgen output dir")?;

        // Build up args for calling wasm-bindgen.
        let arg_target = format!("--target={}", if self.cfg.module_workers { "web" } else { "no-modules" });
        let arg_out_path = format!("--out-dir={}", bindgen_out.display());
        let arg_out_name = format!("--out-name={}", &hashed_name);
        let target_wasm = wasm.to_string_lossy().to_string();
        let mut args = vec!["--no-typescript", &arg_target, &arg_out_path, &arg_out_name, &target_wasm];
        if self.keep_debug {
            args.push("--keep-debug");
        }
        if self.no_demangle {
            args.push("--no-demangle");
        }

        // Invoke wasm-bindgen.
        run_command("wasm-bindgen", &args).await?;

        // Copy the generated WASM & JS loader to the dist dir.
        tracing::info!("copying generated wasm-bindgen artifacts");
        let hashed_js_name = format!("{}.js", &hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &hashed_name);
        let js_loader_path = bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = self.cfg.staging_dist.join(&hashed_js_name);
        let wasm_path = bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = self.cfg.staging_dist.join(&hashed_wasm_name);
        fs::copy(js_loader_path, &js_loader_path_dist)
            .await
            .context("error copying JS loader file to stage dir")?;

        fs::copy(wasm_path, wasm_path_dist)
            .await
            .context("error copying wasm file to stage dir")?;

        let worker_wrapper_path = self.cfg.staging_dist.join("worker.js");
        let worker_wrapper = if self.cfg.module_workers {
            format!(
                "import init from '{base}{js}';init('{base}{wasm}');",
                base = self.cfg.public_url,
                js = hashed_js_name,
                wasm = hashed_wasm_name
            )
        } else {
            format!(
                "importScripts('{base}{js}');wasm_bindgen('{base}{wasm}');",
                base = self.cfg.public_url,
                js = hashed_js_name,
                wasm = hashed_wasm_name
            )
        };
        fs::write(worker_wrapper_path, worker_wrapper)
            .await
            .context("error writing worker wrapper")?;

        // Check for any snippets, and copy them over.
        let snippets_dir = bindgen_out.join(SNIPPETS_DIR);
        if path_exists(&snippets_dir).await? {
            copy_dir_recursive(bindgen_out.join(SNIPPETS_DIR), self.cfg.staging_dist.join(SNIPPETS_DIR))
                .await
                .context("error copying snippets dir to stage dir")?;
        }

        Ok(RustWorkerOutput {
            id: self.id,
            cfg: self.cfg.clone(),
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
        })
    }
}

/// The output of a cargo build pipeline for a Rust web worker.
pub struct RustWorkerOutput {
    /// The ID of this pipeline.
    pub id: usize,
    pub cfg: Arc<RtcBuild>,
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}

impl RustWorkerOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&super::trunk_id_selector(self.id)).remove();
        Ok(())
    }
}
