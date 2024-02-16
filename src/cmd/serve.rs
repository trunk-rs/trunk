use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tokio::select;
use tokio::sync::broadcast;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsServe, ConfigOptsWatch};
use crate::serve::ServeSystem;
use crate::version::enforce_version;

/// Build, watch & serve the Rust WASM app and all of its assets.
#[derive(Args)]
#[command(name = "serve")]
pub struct Serve {
    #[command(flatten)]
    pub build: ConfigOptsBuild,
    #[command(flatten)]
    pub watch: ConfigOptsWatch,
    #[command(flatten)]
    pub serve: ConfigOptsServe,
}

impl Serve {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let cfg = ConfigOpts::rtc_serve(self.build, self.watch, self.serve, config).await?;
        enforce_version(&cfg.watch.build.core)?;

        let system = ServeSystem::new(cfg, shutdown_tx.clone()).await?;

        let system_handle = tokio::spawn(system.run());

        select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::debug!("received shutdown signal");
                shutdown_tx.send(()).ok();
                drop(shutdown_tx);
            }
            r = system_handle => {
                r.context("error awaiting system shutdown")??;
            }
        }

        tracing::debug!("Exiting serve main");

        Ok(())
    }
}
