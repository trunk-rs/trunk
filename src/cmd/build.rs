use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::build::BuildSystem;
use crate::config::{ConfigOpts, ConfigOptsBuild};

/// Build the Rust WASM app and all of its assets.
#[derive(Clone, Debug, Args)]
#[clap(name = "build")]
pub struct Build {
    #[clap(flatten)]
    pub build: ConfigOptsBuild,
}

impl Build {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_build(self.build, config)?;
        let mut system = BuildSystem::new(cfg, None).await?;
        system.build().await?;
        Ok(())
    }
}
