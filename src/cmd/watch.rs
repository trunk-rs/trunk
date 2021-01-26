use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::common::spinner;
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
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_watch(self.build, self.watch, config).await?;
        let mut system = WatchSystem::new(cfg, spinner()).await?;
        system.build().await;
        system.run().await?;
        Ok(())
    }
}
