use std::path::PathBuf;

use anyhow::{Context, Result};
use structopt::StructOpt;
use tokio::sync::broadcast;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsWatch};
use crate::watch::WatchSystem;

/// Build & watch the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name = "watch")]
pub struct Watch {
    #[structopt(flatten)]
    pub build: ConfigOptsBuild,
    #[structopt(flatten)]
    pub watch: ConfigOptsWatch,
}

impl Watch {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
        let cfg = ConfigOpts::rtc_watch(self.build, self.watch, config)?;
        let mut system = WatchSystem::new(cfg, shutdown_tx.clone(), None).await?;

        let _res = system.build().await;
        let system_handle = tokio::spawn(system.run());
        let _res = tokio::signal::ctrl_c().await.context("error awaiting shutdown signal")?;
        tracing::debug!("received shutdown signal");
        let _res = shutdown_tx.send(());
        drop(shutdown_tx); // Ensure other components see the drop to avoid race conditions.
        system_handle.await.context("error awaiting system shutdown")?;

        Ok(())
    }
}
