use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use crate::common::parse_public_url;
use crate::watch::WatchSystem;

/// Watch the Rust WASM app and execute builds as changes are detected.
#[derive(Clap)]
#[clap(name="watch")]
pub struct Watch {
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
    /// Additional paths to ignore.
    #[clap(short, long, parse(from_os_str), env="IGNORE")]
    ignore: Option<Vec<PathBuf>>,
    /// Path to Cargo.toml.
    #[clap(long="manifest-path", parse(from_os_str), env="MANIFEST_PATH")]
    manifest: Option<PathBuf>,
}

impl Watch {
    pub async fn run(self) -> Result<()> {
        let mut system = WatchSystem::new(
            self.target, self.release, self.dist, self.public_url,
            self.ignore.unwrap_or_default(), self.manifest,
        ).await?;
        system.build().await;
        system.run().await;
        Ok(())
    }
}
