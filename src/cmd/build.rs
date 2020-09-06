use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use crate::build::{BuildSystem, CargoMetadata};
use crate::common::parse_public_url;

/// Build the Rust WASM app and all of its assets.
#[derive(Clap)]
#[clap(name="build")]
pub struct Build {
    /// The index HTML file to drive the bundling process.
    #[clap(default_value="index.html", parse(from_os_str), env="TARGET")]
    target: PathBuf,
    /// Build in release mode.
    #[clap(long, env="RELEASE")]
    release: bool,
    /// The output dir for all final assets.
    #[clap(short, long, default_value="dist", parse(from_os_str), env="DIST")]
    dist: PathBuf,
    /// The public URL from which assets are to be served.
    #[clap(long, default_value="/", parse(from_str=parse_public_url), env="PUBLIC_URL")]
    public_url: String,
    /// Path to Cargo.toml.
    #[clap(long="manifest-path", parse(from_os_str), env="MANIFEST_PATH")]
    manifest: Option<PathBuf>,
}

impl Build {
    pub async fn run(self) -> Result<()> {
        let manifest = CargoMetadata::new(&self.manifest).await?;
        let mut system = BuildSystem::new(
            manifest, self.target.clone(), self.release,
            self.dist.clone(), self.public_url.clone(),
        ).await?;
        system.build().await?;
        Ok(())
    }
}
