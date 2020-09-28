//! Runtime config.
//!
//! Trunk takes the typical layered configuration approach. There are 3 layers. The
//! `Trunk.toml` config file is the base, which is then superseded by environment variables,
//! which are finally superseded by CLI arguments and options.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use http_types::Url;
use serde::Deserialize;
use structopt::StructOpt;

use crate::build::CargoMetadata;
use crate::common::parse_public_url;

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// The metadata of the associated cargo project being processed.
    pub manifest: CargoMetadata,
    /// The public URL from which assets are to be served.
    pub public_url: String,
}

impl From<(CargoMetadata, ConfigOptsBuild)> for RtcBuild {
    fn from((manifest, opts): (CargoMetadata, ConfigOptsBuild)) -> Self {
        Self {
            target: opts.target.unwrap_or_else(|| "index.html".into()),
            release: opts.release,
            dist: opts.dist.unwrap_or_else(|| "dist".into()),
            manifest,
            public_url: opts.public_url.unwrap_or_else(|| "/".into()),
        }
    }
}

/// Runtime config for the watch system.
#[derive(Clone, Debug)]
pub struct RtcWatch {
    /// Runtime config for the build system.
    pub build: Arc<RtcBuild>,
    /// Additional paths to ignore.
    pub ignore: Vec<PathBuf>,
}

impl From<(CargoMetadata, ConfigOptsBuild, ConfigOptsWatch)> for RtcWatch {
    fn from((manifest, build_opts, opts): (CargoMetadata, ConfigOptsBuild, ConfigOptsWatch)) -> Self {
        let build = Arc::new(RtcBuild::from((manifest, build_opts)));
        Self {
            build,
            ignore: opts.ignore.unwrap_or_default(),
        }
    }
}

/// Runtime config for the serve system.
#[derive(Clone, Debug)]
pub struct RtcServe {
    /// Runtime config for the watch system.
    pub watch: Arc<RtcWatch>,
    /// The port to serve on.
    pub port: u16,
    /// Open a browser tab once the initial build is complete.
    pub open: bool,
    /// A URL to which requests will be proxied.
    pub proxy_backend: Option<Url>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend.
    pub proxy_rewrite: Option<String>,
    /// Any proxies configured to run along with the server.
    pub proxies: Option<Vec<ConfigOptsProxy>>,
}

impl
    From<(
        CargoMetadata,
        ConfigOptsBuild,
        ConfigOptsWatch,
        ConfigOptsServe,
        Option<Vec<ConfigOptsProxy>>,
    )> for RtcServe
{
    fn from(
        (manifest, build_opts, watch_opts, opts, proxies): (
            CargoMetadata,
            ConfigOptsBuild,
            ConfigOptsWatch,
            ConfigOptsServe,
            Option<Vec<ConfigOptsProxy>>,
        ),
    ) -> Self {
        let watch = Arc::new(RtcWatch::from((manifest, build_opts, watch_opts)));
        Self {
            watch,
            port: opts.port.unwrap_or(8080),
            open: opts.open,
            proxy_backend: opts.proxy_backend,
            proxy_rewrite: opts.proxy_rewrite,
            proxies,
        }
    }
}

/// Runtime config for the clean system.
#[derive(Clone, Debug)]
pub struct RtcClean {
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
}

impl From<ConfigOptsClean> for RtcClean {
    fn from(opts: ConfigOptsClean) -> Self {
        Self {
            dist: opts.dist.unwrap_or_else(|| "dist".into()),
            cargo: opts.cargo,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

/// Config options for the build system.
#[derive(Clone, Debug, Default, Deserialize, StructOpt)]
pub struct ConfigOptsBuild {
    /// The index HTML file to drive the bundling process [default: index.html]
    #[structopt(parse(from_os_str))]
    pub target: Option<PathBuf>,
    /// Build in release mode [default: false]
    #[structopt(long)]
    #[serde(default)]
    pub release: bool,
    /// The output dir for all final assets [default: dist]
    #[structopt(short, long, parse(from_os_str))]
    pub dist: Option<PathBuf>,
    /// The public URL from which assets are to be served [default: /]
    #[structopt(long, parse(from_str=parse_public_url))]
    pub public_url: Option<String>,
    /// Path to Cargo.toml [default: Cargo.toml]
    #[structopt(long = "manifest-path", parse(from_os_str))]
    pub manifest: Option<PathBuf>,
}

/// Config options for the watch system.
#[derive(Clone, Debug, Default, Deserialize, StructOpt)]
pub struct ConfigOptsWatch {
    /// Additional paths to ignore [default: []]
    #[structopt(short, long, parse(from_os_str))]
    pub ignore: Option<Vec<PathBuf>>,
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, StructOpt)]
pub struct ConfigOptsServe {
    /// The port to serve on [default: 8080]
    #[structopt(long)]
    pub port: Option<u16>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[structopt(long)]
    #[serde(default)]
    pub open: bool,
    /// A URL to which requests will be proxied [default: None]
    #[structopt(long = "proxy-backend")]
    #[serde(default)]
    pub proxy_backend: Option<Url>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[structopt(long = "proxy-rewrite")]
    #[serde(default)]
    pub proxy_rewrite: Option<String>,
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, StructOpt)]
pub struct ConfigOptsClean {
    /// The output dir for all final assets [default: dist]
    #[structopt(short, long, parse(from_os_str))]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean [default: false]
    #[structopt(long)]
    #[serde(default)]
    pub cargo: bool,
}

/// Config options for building proxies.
///
/// NOTE WELL: this configuration type is different from the others inasmuch as it is only used
/// when parsing the `Trunk.toml` config file. It is not intended to be configured via CLI or env
/// vars.
#[derive(Clone, Debug, Deserialize)]
pub struct ConfigOptsProxy {
    /// The URL of the backend to which requests are to be proxied.
    pub backend: Url,
    /// An optional URI prefix which is to be used as the base URI for proxying requests, which
    /// defaults to the URI of the backend.
    ///
    /// When a value is specified, requests received on this URI will have this URI segment replaced
    /// with the URI of the `backend`.
    pub rewrite: Option<String>,
}

/// A model of all potential configuration options for the Trunk CLI system.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOpts {
    pub build: Option<ConfigOptsBuild>,
    pub watch: Option<ConfigOptsWatch>,
    pub serve: Option<ConfigOptsServe>,
    pub clean: Option<ConfigOptsClean>,
    pub proxy: Option<Vec<ConfigOptsProxy>>,
}

impl ConfigOpts {
    /// Extract the runtime config for the build system based on all config layers.
    pub async fn rtc_build(cli_build: ConfigOptsBuild, config: Option<PathBuf>) -> Result<Arc<RtcBuild>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let build_opts = build_layer.build.unwrap_or_default();
        let manifest = CargoMetadata::new(&build_opts.manifest).await?;
        Ok(Arc::new(RtcBuild::from((manifest, build_opts))))
    }

    /// Extract the runtime config for the watch system based on all config layers.
    pub async fn rtc_watch(cli_build: ConfigOptsBuild, cli_watch: ConfigOptsWatch, config: Option<PathBuf>) -> Result<Arc<RtcWatch>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let build_opts = watch_layer.build.unwrap_or_default();
        let watch_opts = watch_layer.watch.unwrap_or_default();
        let manifest = CargoMetadata::new(&build_opts.manifest).await?;
        Ok(Arc::new(RtcWatch::from((manifest, build_opts, watch_opts))))
    }

    /// Extract the runtime config for the serve system based on all config layers.
    pub async fn rtc_serve(
        cli_build: ConfigOptsBuild, cli_watch: ConfigOptsWatch, cli_serve: ConfigOptsServe, config: Option<PathBuf>,
    ) -> Result<Arc<RtcServe>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let serve_layer = Self::cli_opts_layer_serve(cli_serve, watch_layer);
        let build_opts = serve_layer.build.unwrap_or_default();
        let watch_opts = serve_layer.watch.unwrap_or_default();
        let serve_opts = serve_layer.serve.unwrap_or_default();
        let manifest = CargoMetadata::new(&build_opts.manifest).await?;
        Ok(Arc::new(RtcServe::from((
            manifest,
            build_opts,
            watch_opts,
            serve_opts,
            serve_layer.proxy,
        ))))
    }

    /// Extract the runtime config for the clean system based on all config layers.
    pub async fn rtc_clean(cli_clean: ConfigOptsClean, config: Option<PathBuf>) -> Result<Arc<RtcClean>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let clean_layer = Self::cli_opts_layer_clean(cli_clean, base_layer);
        let clean_opts = clean_layer.clean.unwrap_or_default();
        Ok(Arc::new(RtcClean::from(clean_opts)))
    }

    /// Return the full configuration based on config file & environment variables.
    pub async fn full(config: Option<PathBuf>) -> Result<Self> {
        Self::file_and_env_layers(config)
    }

    fn cli_opts_layer_build(cli: ConfigOptsBuild, cfg_base: Self) -> Self {
        let opts = ConfigOptsBuild {
            target: cli.target,
            release: cli.release,
            dist: cli.dist,
            manifest: cli.manifest,
            public_url: cli.public_url,
        };
        let cfg_build = ConfigOpts {
            build: Some(opts),
            watch: None,
            serve: None,
            clean: None,
            proxy: None,
        };
        Self::merge(cfg_base, cfg_build)
    }

    fn cli_opts_layer_watch(cli: ConfigOptsWatch, cfg_base: Self) -> Self {
        let opts = ConfigOptsWatch { ignore: cli.ignore };
        let cfg = ConfigOpts {
            build: None,
            watch: Some(opts),
            serve: None,
            clean: None,
            proxy: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn cli_opts_layer_serve(cli: ConfigOptsServe, cfg_base: Self) -> Self {
        let opts = ConfigOptsServe {
            port: cli.port,
            open: cli.open,
            proxy_backend: cli.proxy_backend,
            proxy_rewrite: cli.proxy_rewrite,
        };
        let cfg = ConfigOpts {
            build: None,
            watch: None,
            serve: Some(opts),
            clean: None,
            proxy: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn cli_opts_layer_clean(cli: ConfigOptsClean, cfg_base: Self) -> Self {
        let opts = ConfigOptsClean {
            dist: cli.dist,
            cargo: cli.cargo,
        };
        let cfg = ConfigOpts {
            build: None,
            watch: None,
            serve: None,
            clean: Some(opts),
            proxy: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn file_and_env_layers(path: Option<PathBuf>) -> Result<Self> {
        let toml_cfg = Self::from_file(path)?;
        let env_cfg = Self::from_env()?;
        let cfg = Self::merge(toml_cfg, env_cfg);
        Ok(cfg)
    }

    fn from_file(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(|| "Trunk.toml".into());
        if path.exists() {
            let cfg_bytes = std::fs::read(path)?;
            let cfg: Self = toml::from_slice(&cfg_bytes)?;
            Ok(cfg)
        } else {
            Ok(Default::default())
        }
    }

    fn from_env() -> Result<Self> {
        let build: ConfigOptsBuild = envy::prefixed("TRUNK_BUILD_").from_env()?;
        let watch: ConfigOptsWatch = envy::prefixed("TRUNK_WATCH_").from_env()?;
        let serve: ConfigOptsServe = envy::prefixed("TRUNK_SERVE_").from_env()?;
        let clean: ConfigOptsClean = envy::prefixed("TRUNK_CLEAN_").from_env()?;
        Ok(ConfigOpts {
            build: Some(build),
            watch: Some(watch),
            serve: Some(serve),
            clean: Some(clean),
            proxy: None,
        })
    }

    /// Merge the given layers, where the `greater` layer takes precedence.
    fn merge(mut lesser: Self, mut greater: Self) -> Self {
        greater.build = match (lesser.build.take(), greater.build.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.target = g.target.or(l.target);
                g.dist = g.dist.or(l.dist);
                g.manifest = g.manifest.or(l.manifest);
                g.public_url = g.public_url.or(l.public_url);
                // NOTE: this can not be disabled in the cascade.
                if l.release {
                    g.release = true
                }
                Some(g)
            }
        };
        greater.watch = match (lesser.watch.take(), greater.watch.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.ignore = g.ignore.or(l.ignore);
                Some(g)
            }
        };
        greater.serve = match (lesser.serve.take(), greater.serve.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.proxy_backend = g.proxy_backend.or(l.proxy_backend);
                g.proxy_rewrite = g.proxy_rewrite.or(l.proxy_rewrite);
                g.port = g.port.or(l.port);
                // NOTE: this can not be disabled in the cascade.
                if l.open {
                    g.open = true
                }
                Some(g)
            }
        };
        greater.clean = match (lesser.clean.take(), greater.clean.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.dist = g.dist.or(l.dist);
                // NOTE: this can not be disabled in the cascade.
                if l.cargo {
                    g.cargo = true
                }
                Some(g)
            }
        };
        greater.proxy = match (lesser.proxy.take(), greater.proxy.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(_), Some(g)) => Some(g), // No meshing/merging. Only take the greater value.
        };
        greater
    }
}
