//! Build system & asset pipelines.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_std::fs;
use async_std::path::Path;
use futures::channel::mpsc::Sender;
use futures::stream::StreamExt;
use indicatif::ProgressBar;

use crate::common::{remove_dir_all, BUILDING, ERROR, SUCCESS};
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

    /// Creates a "staging area" (dist/.stage) for storing intermediate build results
    async fn prepare_dist_staging(&self) -> Result<()> {
        // Prepare staging area in which we will assemble the latest build
        let dist_staging: &Path = self.cfg.dist_staging.as_path().into();

        // Clean staging area, if applicable
        remove_dir_all(dist_staging.into()).await.context("error cleaning staging dist dir")?;
        fs::create_dir_all(dist_staging)
            .await
            .with_context(|| "error creating build environment directory: staging dist dir")?;

        Ok(())
    }

    async fn do_build(&mut self) -> Result<()> {
        // Ensure the output dist directories are in place.
        fs::create_dir_all(self.cfg.dist_final.as_path())
            .await
            .with_context(|| "error creating build environment directory: dist")?;

        self.prepare_dist_staging().await.context("error preparing build environment")?;

        // Spawn the source HTML pipeline. This will spawn all other pipelines derived from
        // the source HTML, and will ultimately generate and write the final HTML.
        self.html_pipeline.clone().spawn().await?;

        // Move distrbution from staging dist to final dist
        self.finalize_dist().await.context("error applying built distribution")?;
        Ok(())
    }

    /// Moves the contents of dist/.stage into dist, signifying the application
    /// of a successful build. Also removes dist/.stage afterwards.
    async fn finalize_dist(&self) -> Result<()> {
        let dist_final = self.cfg.dist_final.clone();
        let dist_staging = self.cfg.dist_staging.clone();
        self.progress.clone().set_message("applying new distribution");

        // Build succeeded, so delete everything in `dist`,
        // move everything from `dist/.stage` to `dist`, and
        // then delete `dist/.stage`.
        let mut entries = fs::read_dir(&dist_final).await.context("error reading final dist dir")?;
        while let Some(entry) = entries.next().await {
            let entry = entry.context("error reading contents of final dist dir")?;
            if entry.file_name() == ".stage" {
                continue;
            }
            let target_path = dist_final.join(entry.file_name());

            fs::rename(entry.path(), target_path)
                .await
                .context("error moving from staging dir to dist dir")?;
        }

        fs::remove_dir(dist_staging).await.context("error deleting staging dist dir")?;

        Ok(())
    }
}
