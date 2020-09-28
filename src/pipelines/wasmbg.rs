//! WASM bindgen pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use async_process::{Command, Stdio};
use async_std::fs;
use async_std::task::{spawn, JoinHandle};
use indicatif::ProgressBar;

use crate::common::copy_dir_recursive;
use crate::config::RtcBuild;
use crate::pipelines::cargo::CargoBuildOutput;

const SNIPPETS_DIR: &str = "snippets";

/// A wasm-bindgen build pipeline.
pub struct WasmBindgen {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The output dir of the wasm-bindgen execution.
    bindgen_out: Arc<PathBuf>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
}

impl WasmBindgen {
    /// Create a new instance.
    pub fn new(cfg: Arc<RtcBuild>, bindgen_out: Arc<PathBuf>, progress: ProgressBar) -> Self {
        Self { cfg, bindgen_out, progress }
    }

    /// Spawn a new pipeline.
    pub fn spawn(self: Arc<Self>, cargo: JoinHandle<Result<CargoBuildOutput>>) -> JoinHandle<Result<WasmBindgenOutput>> {
        spawn(self.build(cargo))
    }

    /// Perform the build routine of this pipeline.
    async fn build(self: Arc<Self>, cargo: JoinHandle<Result<CargoBuildOutput>>) -> Result<WasmBindgenOutput> {
        self.progress.set_message("awaiting cargo build");
        let cargo = cargo.await?;
        self.progress.set_message("executing");
        let arg_out_path = format!("--out-dir={}", self.bindgen_out.display());
        let arg_out_name = format!("--out-name={}", &cargo.hashed_name);
        let target_wasm = cargo.wasm.to_string_lossy().to_string();

        // Ensure our output dir is in place.
        fs::create_dir_all(self.bindgen_out.as_path())
            .await
            .context("error creating wasm-bindgen output dir")?;

        // Spawn the wasm-bindgen process.
        let args = vec!["--target=web", &arg_out_path, &arg_out_name, "--no-typescript", &target_wasm];
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
        let hashed_js_name = format!("{}.js", &cargo.hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &cargo.hashed_name);
        let js_loader_path = self.bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = self.cfg.dist.join(&hashed_js_name);
        let wasm_path = self.bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = self.cfg.dist.join(&hashed_wasm_name);
        fs::copy(js_loader_path, js_loader_path_dist)
            .await
            .context("error copying JS loader file to dist dir")?;
        fs::copy(wasm_path, wasm_path_dist).await.context("error copying wasm file to dist dir")?;

        // Check for any snippets, and copy them over.
        copy_dir_recursive(self.bindgen_out.join(SNIPPETS_DIR), self.cfg.dist.join(SNIPPETS_DIR))
            .await
            .context("error copying snippets dir")?;

        Ok(WasmBindgenOutput {
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
        })
    }
}

/// The output of the wasm-bindgen process.
pub struct WasmBindgenOutput {
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}
