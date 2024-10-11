use crate::{
    config::{
        self,
        rt::{self, RtcBuilder, RtcWatch},
        types::ConfigDuration,
        Configuration,
    },
    watch::WatchSystem,
};
use anyhow::{Context, Result};
use clap::Args;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;

/// Build & watch the Rust WASM app and all of its assets.
#[derive(Clone, Args)]
#[command(name = "watch")]
#[command(next_help_heading = "Watch")]
pub struct Watch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[arg(short, long, value_name = "path", env = "TRUNK_WATCH_WATCH")]
    pub watch: Option<Vec<PathBuf>>,
    /// Paths to ignore [default: []]
    #[arg(short, long, value_name = "path", env = "TRUNK_WATCH_IGNORE")]
    pub ignore: Option<Vec<PathBuf>>,
    /// Using polling mode for detecting changes
    #[arg(long, env = "TRUNK_WATCH_POLL")]
    pub poll: bool,
    /// The polling interval, when polling is enabled
    #[arg(long, env = "TRUNK_WATCH_POLL_INTERVAL", default_value = "5s")]
    pub poll_interval: ConfigDuration,
    /// Allow enabling a cooldown, discarding all change events during the build
    #[arg(long, env = "TRUNK_WATCH_ENABLE_COOLDOWN")]
    pub enable_cooldown: bool,
    /// Clear the screen before each run
    #[arg(short, long = "clear", env = "TRUNK_WATCH_CLEAR")]
    pub clear_screen: bool,

    // NOTE: flattened structures come last
    #[command(flatten)]
    pub build: super::build::Build,
}

impl Watch {
    // apply CLI overrides to the configuration
    pub fn apply_to(self, mut config: Configuration) -> Result<Configuration> {
        let Self {
            watch,
            ignore,
            poll: _,
            poll_interval: _,
            enable_cooldown: _,
            clear_screen: _,
            build,
        } = self;

        config.watch.watch = watch.unwrap_or(config.watch.watch);
        config.watch.ignore = ignore.unwrap_or(config.watch.ignore);

        let config = build.apply_to(config)?;

        Ok(config)
    }

    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (cfg, working_directory) = config::load(config).await?;

        let cfg = self.clone().apply_to(cfg)?;
        let cfg = RtcWatch::from_config(cfg, working_directory, |_, core| rt::WatchOptions {
            build: rt::BuildOptions {
                core,
                inject_autoloader: false,
            },
            poll: self.poll.then_some(self.poll_interval.0),
            enable_cooldown: self.enable_cooldown,
            clear_screen: self.clear_screen,
            // in watch mode we can't report errors
            no_error_reporting: false,
        })
        .await?;

        cfg.enforce_version()?;

        let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);

        let mut system = WatchSystem::new(Arc::new(cfg), shutdown_tx.clone(), None, None).await?;

        system.build().await.ok();
        let system_handle = tokio::spawn(system.run());
        tokio::signal::ctrl_c()
            .await
            .context("error awaiting shutdown signal")?;
        tracing::debug!("received shutdown signal");
        shutdown_tx.send(()).ok();
        drop(shutdown_tx); // Ensure other components see the drop to avoid race conditions.
        system_handle
            .await
            .context("error awaiting system shutdown")?;

        Ok(())
    }
}
