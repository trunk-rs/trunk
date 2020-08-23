use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::build::{BuildSystem, CargoManifest};

/// Build the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name="build")]
pub struct Build {
    /// The index HTML file to drive the bundling process.
    #[structopt(parse(from_os_str))]
    target: PathBuf,

    /// Build in release mode.
    #[structopt(long)]
    release: bool,
    /// The output dir for all final assets.
    #[structopt(short, long, default_value="dist", parse(from_os_str))]
    dist: PathBuf,
    /// The public URL from which assets are to be served.
    #[structopt(short, long, default_value="/")]
    public_url: String,
}

impl Build {
    pub async fn run(&self) -> Result<()> {
        let manifest = CargoManifest::read_cwd_manifest().await?;
        let mut system = BuildSystem::new(
            manifest, self.target.clone(), self.release,
            self.dist.clone(), self.public_url.clone(),
        ).await?;
        system.build_app().await?;
        Ok(())
    }
}
