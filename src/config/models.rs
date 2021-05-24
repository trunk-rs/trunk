use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use http_types::Url;
use serde::Deserialize;
use structopt::StructOpt;

use crate::common::parse_public_url;
use crate::config::{RtcBuild, RtcClean, RtcServe, RtcWatch};

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
    /// include paths to be passed to libsass [default: "."]
    #[structopt(short = "I", long = "include", parse(from_os_str), value_name = "path")]
    pub includes: Option<Vec<PathBuf>>,
}

/// Config options for the watch system.
#[derive(Clone, Debug, Default, Deserialize, StructOpt)]
pub struct ConfigOptsWatch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[structopt(short, long, parse(from_os_str), value_name = "path")]
    pub watch: Option<Vec<PathBuf>>,
    /// Paths to ignore [default: []]
    #[structopt(short, long, parse(from_os_str), value_name = "path")]
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
    /// Configure the proxy for handling WebSockets [default: false]
    #[structopt(long = "proxy-ws")]
    #[serde(default)]
    pub proxy_ws: bool,
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
    /// Configure the proxy for handling WebSockets.
    #[serde(default)]
    pub ws: bool,
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
    pub fn rtc_build(cli_build: ConfigOptsBuild, config: Option<PathBuf>) -> Result<Arc<RtcBuild>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let build_opts = build_layer.build.unwrap_or_default();
        Ok(Arc::new(RtcBuild::new(build_opts)?))
    }

    /// Extract the runtime config for the watch system based on all config layers.
    pub fn rtc_watch(cli_build: ConfigOptsBuild, cli_watch: ConfigOptsWatch, config: Option<PathBuf>) -> Result<Arc<RtcWatch>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let build_opts = watch_layer.build.unwrap_or_default();
        let watch_opts = watch_layer.watch.unwrap_or_default();
        Ok(Arc::new(RtcWatch::new(build_opts, watch_opts)?))
    }

    /// Extract the runtime config for the serve system based on all config layers.
    pub fn rtc_serve(
        cli_build: ConfigOptsBuild, cli_watch: ConfigOptsWatch, cli_serve: ConfigOptsServe, config: Option<PathBuf>,
    ) -> Result<Arc<RtcServe>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let serve_layer = Self::cli_opts_layer_serve(cli_serve, watch_layer);
        let build_opts = serve_layer.build.unwrap_or_default();
        let watch_opts = serve_layer.watch.unwrap_or_default();
        let serve_opts = serve_layer.serve.unwrap_or_default();
        Ok(Arc::new(RtcServe::new(build_opts, watch_opts, serve_opts, serve_layer.proxy)?))
    }

    /// Extract the runtime config for the clean system based on all config layers.
    pub fn rtc_clean(cli_clean: ConfigOptsClean, config: Option<PathBuf>) -> Result<Arc<RtcClean>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let clean_layer = Self::cli_opts_layer_clean(cli_clean, base_layer);
        let clean_opts = clean_layer.clean.unwrap_or_default();
        Ok(Arc::new(RtcClean::new(clean_opts)))
    }

    /// Return the full configuration based on config file & environment variables.
    pub fn full(config: Option<PathBuf>) -> Result<Self> {
        Self::file_and_env_layers(config)
    }

    fn cli_opts_layer_build(cli: ConfigOptsBuild, cfg_base: Self) -> Self {
        let opts = ConfigOptsBuild {
            target: cli.target,
            release: cli.release,
            dist: cli.dist,
            public_url: cli.public_url,
            includes: cli.includes,
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
        let opts = ConfigOptsWatch {
            watch: cli.watch,
            ignore: cli.ignore,
        };
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
            proxy_ws: cli.proxy_ws,
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
        let env_cfg = Self::from_env().context("error reading trunk env var config")?;
        let cfg = Self::merge(toml_cfg, env_cfg);
        Ok(cfg)
    }

    /// Read runtime config from a `Trunk.toml` file at the target path.
    ///
    /// NOTE WELL: any paths specified in a Trunk.toml file must be interpreted as being relative
    /// to the file itself.
    fn from_file(path: Option<PathBuf>) -> Result<Self> {
        let mut trunk_toml_path = path.unwrap_or_else(|| "Trunk.toml".into());
        if !trunk_toml_path.exists() {
            return Ok(Default::default());
        }
        if !trunk_toml_path.is_absolute() {
            trunk_toml_path = trunk_toml_path
                .canonicalize()
                .with_context(|| format!("error getting canonical path to Trunk config file {:?}", &trunk_toml_path))?;
        }
        let cfg_bytes = std::fs::read(&trunk_toml_path).context("error reading config file")?;
        let mut cfg: Self = toml::from_slice(&cfg_bytes).context("error reading config file contents as TOML data")?;
        if let Some(parent) = trunk_toml_path.parent() {
            if let Some(build) = cfg.build.as_mut() {
                if let Some(target) = build.target.as_mut() {
                    if !target.is_absolute() {
                        *target = std::fs::canonicalize(parent.join(&target))
                            .with_context(|| format!("error taking canonical path to [build].target {:?} in {:?}", target, trunk_toml_path))?;
                    }
                }
                if let Some(dist) = build.dist.as_mut() {
                    if !dist.is_absolute() {
                        *dist = parent.join(&dist);
                    }
                }
                if let Some(includes) = build.includes.as_mut() {
                    for path in includes.iter_mut() {
                        if !path.is_absolute() {
                            *path = std::fs::canonicalize(parent.join(&path))
                                .with_context(|| format!("error taking canonical path to [build].includes {:?} in {:?}", path, trunk_toml_path))?;
                        }
                    }
                }
            }
            if let Some(watch) = cfg.watch.as_mut() {
                if let Some(watch_paths) = watch.watch.as_mut() {
                    for path in watch_paths.iter_mut() {
                        if !path.is_absolute() {
                            *path = std::fs::canonicalize(parent.join(&path))
                                .with_context(|| format!("error taking canonical path to [watch].watch {:?} in {:?}", path, trunk_toml_path))?;
                        }
                    }
                }
                if let Some(ignore_paths) = watch.ignore.as_mut() {
                    for path in ignore_paths.iter_mut() {
                        if !path.is_absolute() {
                            *path = std::fs::canonicalize(parent.join(&path))
                                .with_context(|| format!("error taking canonical path to [watch].ignore {:?} in {:?}", path, trunk_toml_path))?;
                        }
                    }
                }
            }
            if let Some(clean) = cfg.clean.as_mut() {
                if let Some(dist) = clean.dist.as_mut() {
                    if !dist.is_absolute() {
                        *dist = parent.join(&dist);
                    }
                }
            }
        }
        Ok(cfg)
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
                g.public_url = g.public_url.or(l.public_url);
                g.includes = g.includes.or(l.includes);
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
                g.watch = g.watch.or(l.watch);
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
                g.proxy_ws = g.proxy_ws || l.proxy_ws;
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
