use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use crate::common::parse_public_url;
use crate::watch::WatchSystem;
use crate::config::Config;

/// Watch the Rust WASM app and execute builds as changes are detected.
#[derive(Clap)]
#[clap(name = "watch")]
pub struct Watch {
    /// The index HTML file to drive the bundling process. [default: index.html]
    #[clap(parse(from_os_str), env = "TARGET")]
    target: Option<PathBuf>,
    /// Build in release mode.
    #[clap(long, env = "RELEASE")]
    release: bool,
    /// The output dir for all final assets. [default: dist]
    #[clap(short, long, parse(from_os_str), env = "DIST")]
    dist: Option<PathBuf>,
    /// The public URL from which assets are to be served. [default: /]
    #[clap(long, parse(from_str = parse_public_url), env = "PUBLIC_URL")]
    public_url: Option<String>,
    /// Additional paths to ignore.
    #[clap(short, long, parse(from_os_str), env = "IGNORED_PATHS")]
    ignore: Option<Vec<PathBuf>>,
    /// Path to Cargo.toml.
    #[clap(long = "manifest-path", parse(from_os_str), env = "MANIFEST_PATH")]
    manifest: Option<PathBuf>,
}

impl Watch {
    pub async fn run(self, config: Config) -> Result<()> {
        let conf = WatchConfig::new(self, config);
        let mut system = WatchSystem::new(
            conf.target, conf.release, conf.dist, conf.public_url,
            conf.ignored_paths.unwrap_or_default(), conf.manifest,
        ).await?;
        system.build().await;
        system.run().await;
        Ok(())
    }
}

struct WatchConfig {
    target: PathBuf,
    release: bool,
    dist: PathBuf,
    public_url: String,
    ignored_paths: Option<Vec<PathBuf>>,
    manifest: Option<PathBuf>,
}

impl WatchConfig {
    fn new(watch: Watch, toml_config: Config) -> Self {
        let target = if let Some(target) = watch.target { target } else { toml_config.html_target };
        let release = watch.release || toml_config.release;
        let dist = if let Some(dist) = watch.dist { dist } else { toml_config.dist };
        let public_url = if let Some(pub_url) = watch.public_url { pub_url } else { toml_config.public_url };
        let ignored_paths = if let Some(ignores) = watch.ignore { Some(ignores) } else { toml_config.watch.ignored_paths };
        let manifest = if let Some(manifest) = watch.manifest { Some(manifest) } else { toml_config.manifest };

        Self { target, release, dist, public_url, ignored_paths, manifest }
    }
}
