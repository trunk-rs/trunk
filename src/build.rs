//! Build system & asset pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use futures::channel::mpsc::Sender;
use indicatif::ProgressBar;

use crate::common::{BUILDING, ERROR, SUCCESS};
use crate::config::RtcBuild;
use crate::pipelines::HtmlPipeline;

/// A system used for building a Rust WASM app & bundling its assets.
///
/// This unit of data should be used throughout the system for driving build processes and
/// bundling tasks. Different CLI commands which need to trigger builds in some way should
/// be able to gather the needed data to create an instance of this struct, and then the vairous
/// build routines can be cleanly abstracted away form any specific CLI endpoints.
pub struct BuildSystem {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// HTML build pipeline.
    html_pipeline: Arc<HtmlPipeline>,
    /// The build system progress bar for displaying the state of the build system overall.
    progress: ProgressBar,
}

impl BuildSystem {
    /// Create a new instance from the raw components.
    ///
    /// Reducing the number of assumptions here should help us to stay flexible when adding new
    /// commands, rafctoring and the like.
    pub async fn new(cfg: Arc<RtcBuild>, progress: ProgressBar, ignore_chan: Option<Sender<PathBuf>>) -> Result<Self> {
        let html_pipeline = Arc::new(HtmlPipeline::new(cfg.clone(), progress.clone(), ignore_chan)?);
        Ok(Self {
            cfg,
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
                Err(err)
            }
        }
    }

    async fn do_build(&mut self) -> Result<()> {
        // Spawn the source HTML pipeline. This will spawn all other pipelines derived from
        // the source HTML, and will ultimately generate and write the final HTML.
        self.html_pipeline.clone().spawn().await?;
        Ok(())
    }
}
