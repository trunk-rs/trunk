//! Rust web worker pipeline.

#![allow(dead_code, unused_variables)] // TODO: remove this when this pipeline type is implemented.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use indicatif::ProgressBar;
use nipper::Document;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use super::{LinkAttrs, TrunkLinkPipelineOutput};
use crate::config::{CargoMetadata, RtcBuild};

/// A Rust web worker pipeline.
pub struct RustWorker {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate ignore paths to the watcher.
    ignore_chan: Option<Sender<PathBuf>>,
}

impl RustWorker {
    pub const TYPE_RUST_WORKER: &'static str = "rust-worker";

    pub async fn new(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>, attrs: LinkAttrs, id: usize,
    ) -> Result<Self> {
        bail!(r#"the rust web worker asset type `<link data-trunk rel="rust-worker" .../>` is not yet supported"#)
    }

    /// Spawn a new pipeline.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        unimplemented!()
    }
}

/// The output of a cargo build pipeline for a Rust web worker.
pub struct RustWorkerOutput {
    /// The ID of this pipeline.
    pub id: Option<usize>,
    pub cfg: Arc<RtcBuild>,
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}

impl RustWorkerOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        unimplemented!()
    }
}
