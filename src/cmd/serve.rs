use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tokio::sync::broadcast;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsServe, ConfigOptsWatch};
use crate::serve::ServeSystem;

/// Build, watch & serve the Rust WASM app and all of its assets.
#[derive(Args)]
#[clap(name = "serve")]
pub struct Serve {
    #[clap(flatten)]
    pub build: ConfigOptsBuild,
    #[clap(flatten)]
    pub watch: ConfigOptsWatch,
    #[clap(flatten)]
    pub serve: ConfigOptsServe,
}

impl Serve {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let cfg = ConfigOpts::rtc_serve(self.build, self.watch, self.serve, config)?;
        let system = ServeSystem::new(cfg, shutdown_tx.clone()).await?;

        let system_handle = tokio::spawn(system.run());
        let _res = tokio::signal::ctrl_c().await.context("error awaiting shutdown signal")?;
        tracing::debug!("received shutdown signal");
        let _res = shutdown_tx.send(());
        drop(shutdown_tx); // Ensure other components see the drop to avoid race conditions.
        system_handle.await.context("error awaiting system shutdown")??;

        Ok(())
    }
}
