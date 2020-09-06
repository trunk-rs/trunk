use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use crate::build::{BuildSystem, CargoMetadata};
use crate::common::parse_public_url;
use crate::config::Config;

/// Build the Rust WASM app and all of its assets.
#[derive(Clap)]
#[clap(name="build")]
pub struct Build {
    /// The index HTML file to drive the bundling process. [default: index.html]
    #[clap(parse(from_os_str), env="TARGET")]
    target: Option<PathBuf>,
    /// Build in release mode.
    #[clap(long, env="RELEASE")]
    release: bool,
    /// The output dir for all final assets. [default: dist]
    #[clap(short, long, parse(from_os_str), env="DIST")]
    dist: Option<PathBuf>,
    /// The public URL from which assets are to be served. [default: /]
    #[clap(long, parse(from_str=parse_public_url), env="PUBLIC_URL")]
    public_url: Option<String>,
    /// Path to Cargo.toml.
    #[clap(long="manifest-path", parse(from_os_str), env="MANIFEST_PATH")]
    manifest: Option<PathBuf>,
}

impl Build {
    pub async fn run(self, config: Config) -> Result<()> {
        let conf = BuildConfig::new(self, config);

        let manifest = CargoMetadata::new(&conf.manifest).await?;
        let mut system = BuildSystem::new(
            manifest, conf.target.clone(), conf.release,
            conf.dist.clone(), conf.public_url.clone(),
        ).await?;
        system.build().await?;
        Ok(())
    }
}

struct BuildConfig {
    target: PathBuf,
    release: bool,
    dist: PathBuf,
    public_url: String,
    manifest: Option<PathBuf>,
}

impl BuildConfig {
    fn new(build: Build, toml_config: Config) -> Self {
        BuildConfig {
            target: build.target.unwrap_or(toml_config.html_target),
            release: build.release || toml_config.release,
            dist: build.dist.unwrap_or(toml_config.dist),
            public_url: build.public_url.unwrap_or(toml_config.public_url),
            manifest: if let Some(manifest) = build.manifest { Some(manifest) } else { toml_config.manifest },
        }
    }
}
