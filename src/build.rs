//! Build system & asset pipelines.

use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use indicatif::ProgressBar;

use crate::common::{BUILDING, ERROR, SUCCESS};
use crate::config::RtcBuild;
use crate::pipelines::cargo::CargoBuild;
use crate::pipelines::html::HtmlPipeline;
use crate::pipelines::wasmbg::WasmBindgen;

/// A system used for building a Rust WASM app & bundling its assets.
///
/// This unit of data should be used throughout the system for driving build processes and
/// bundling tasks. Different CLI commands which need to trigger builds in some way should
/// be able to gather the needed data to create an instance of this struct, and then the vairous
/// build routines can be cleanly abstracted away form any specific CLI endpoints.
pub struct BuildSystem {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// Cargo build pipeline system.
    cargo_build_pipeline: Arc<CargoBuild>,
    /// WASM bindgen build pipeline system.
    wasm_bindgen_pipeline: Arc<WasmBindgen>,
    /// HTML build pipeline system.
    html_pipeline: Arc<HtmlPipeline>,
    /// The build system progress bar for displaying the state of the build system overall.
    progress: ProgressBar,
}

impl BuildSystem {
    /// Create a new instance from the raw components.
    ///
    /// Reducing the number of assumptions here should help us to stay flexible when adding new
    /// commands, rafctoring and the like.
    pub async fn new(cfg: Arc<RtcBuild>, progress: ProgressBar) -> Result<Self> {
        let mode_segment = if cfg.release { "release" } else { "debug" };
        let bindgen_out = Arc::new(cfg.manifest.metadata.target_directory.join("wasm-bindgen").join(mode_segment));

        let cargo_build_pipeline = Arc::new(CargoBuild::new(cfg.clone(), progress.clone()));
        let wasm_bindgen_pipeline = Arc::new(WasmBindgen::new(cfg.clone(), bindgen_out, progress.clone()));
        let html_pipeline = Arc::new(HtmlPipeline::new(cfg.clone(), progress.clone())?);

        Ok(Self {
            cfg,
            cargo_build_pipeline,
            wasm_bindgen_pipeline,
            html_pipeline,
            progress,
        })
    }

    /// Build the application described in the given build data.
    pub async fn build(&mut self) -> Result<()> {
        self.progress.reset();
        self.progress.enable_steady_tick(100);
        self.progress.set_prefix(&format!("{}", BUILDING));
        self.progress.set_message("starting build");
        let res = self.do_build().await;
        self.progress.disable_steady_tick();
        self.progress.set_position(0);
        match res {
            Ok(_) => {
                self.progress.set_prefix(&format!("{}", SUCCESS));
                self.progress.finish_with_message("success");
                Ok(())
            }
            Err(err) => {
                self.progress.set_prefix(&format!("{}", ERROR));
                self.progress.finish_with_message("error");
                self.progress.println(err.to_string());
                Err(err)
            }
        }
    }

    async fn do_build(&mut self) -> Result<()> {
        // Spawn cargo build. It will run concurrently without polling.
        let cargo_handle = self.cargo_build_pipeline.clone().spawn();

        // Ensure the output dist directory is in place.
        fs::create_dir_all(self.cfg.dist.as_path()).await?;

        // Spawn the wasm-bindgen call, it will await the cargo build.
        let wasmbg_handle = self.wasm_bindgen_pipeline.clone().spawn(cargo_handle);

        // Spawn the source HTML pipeline. This will spawn all other asset pipelines derived from
        // the source HTML, and will await the cargo build & wasm-bindgen build in order to
        // generate the final HTML.
        self.html_pipeline.clone().spawn(wasmbg_handle).await?;
        Ok(())
    }
}
