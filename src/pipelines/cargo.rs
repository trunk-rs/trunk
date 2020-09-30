//! Cargo pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use async_process::{Command, Stdio};
use async_std::task::{spawn, JoinHandle};
use indicatif::ProgressBar;

use crate::config::RtcBuild;

/// A cargo build pipeline.
pub struct CargoBuild {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
}

impl CargoBuild {
    /// Create a new instance.
    pub fn new(cfg: Arc<RtcBuild>, progress: ProgressBar) -> Self {
        Self { cfg, progress }
    }

    /// Spawn a new pipeline.
    pub fn spawn(self: Arc<Self>) -> JoinHandle<Result<CargoBuildOutput>> {
        spawn(self.build())
    }

    /// Perform the build routine of this pipeline.
    async fn build(self: Arc<Self>) -> Result<CargoBuildOutput> {
        self.progress.set_message(&format!("building {}", &self.cfg.manifest.package.name));

        // Spawn the cargo build process.
        let mut args = vec![
            "build",
            "--target=wasm32-unknown-unknown",
            "--manifest-path",
            &self.cfg.manifest.manifest_path,
        ];
        if self.cfg.release {
            args.push("--release");
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
                cargo_metadata::Message::CompilerArtifact(art) if art.package_id == self.cfg.manifest.package.id => Ok(Some(art)),
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
        let wasm_bytes = async_std::fs::read(&wasm).await.context("error reading wasm file for hash generation")?;
        let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes));
        Ok(CargoBuildOutput { wasm, hashed_name })
    }
}

/// The output of a cargo build pipeline.
pub struct CargoBuildOutput {
    /// The path to the build WASM file.
    pub wasm: PathBuf,
    /// The hashed name to use for the WASM file.
    pub hashed_name: String,
}
