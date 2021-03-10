use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsServe, ConfigOptsWatch};
use crate::serve::ServeSystem;

/// Build, watch & serve the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name = "serve")]
pub struct Serve {
    #[structopt(flatten)]
    pub build: ConfigOptsBuild,
    #[structopt(flatten)]
    pub watch: ConfigOptsWatch,
    #[structopt(flatten)]
    pub serve: ConfigOptsServe,
}

impl Serve {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_serve(self.build, self.watch, self.serve, config).await?;
        let system = ServeSystem::new(cfg).await?;
        system.run().await?;
        Ok(())
    }
}
