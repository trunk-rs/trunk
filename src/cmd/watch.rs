use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tokio::sync::broadcast;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsWatch};
use crate::watch::WatchSystem;

/// Build & watch the Rust WASM app and all of its assets.
#[derive(Args)]
#[command(name = "watch")]
pub struct Watch {
    #[command(flatten)]
    pub build: ConfigOptsBuild,
    #[command(flatten)]
    pub watch: ConfigOptsWatch,
}

impl Watch {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
        let cfg = ConfigOpts::rtc_watch(self.build, self.watch, config)?;
        let mut system = WatchSystem::new(cfg, shutdown_tx.clone(), None, None).await?;

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
