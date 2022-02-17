use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::http::Uri;
use clap::Args;
use serde::{Deserialize, Deserializer};

use crate::common::parse_public_url;
use crate::config::{RtcBuild, RtcClean, RtcServe, RtcWatch};
use crate::pipelines::PipelineStage;

/// Config options for the build system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsBuild {
    /// The index HTML file to drive the bundling process [default: index.html]
    #[clap(parse(from_os_str))]
    pub target: Option<PathBuf>,
    /// Build in release mode [default: false]
    #[clap(long)]
    #[serde(default)]
    pub release: bool,
    /// The output dir for all final assets [default: dist]
    #[clap(short, long, parse(from_os_str))]
    pub dist: Option<PathBuf>,
    /// The public URL from which assets are to be served [default: /]
    #[clap(long, parse(from_str=parse_public_url))]
    pub public_url: Option<String>,
    /// Build without default features [default: false]
    #[clap(long)]
    #[serde(default)]
    pub no_default_features: bool,
    /// Build with all features [default: false]
    #[clap(long)]
    #[serde(default)]
    pub all_features: bool,
    /// A comma-separated list of features to activate, must not be used with all-features
    /// [default: ""]
    #[clap(long)]
    pub features: Option<String>,
    /// Whether to include hash values in the output file names [default: true]
    #[clap(long)]
    pub filehash: Option<bool>,
    /// Optional pattern for the app loader script [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly load the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[clap(skip)]
    #[serde(default)]
    pub pattern_script: Option<String>,
    /// Optional pattern for the app preload element [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly preload the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[clap(skip)]
    #[serde(default)]
    pub pattern_preload: Option<String>,
    #[clap(skip)]
    #[serde(default)]
    /// Optional replacement parameters corresponding to the patterns provided in
    /// `pattern_script` and `pattern_preload`.
    ///
    /// When a pattern is being replaced with its corresponding value from this map, if the value
    /// is prefixed with the symbol `@`, then the value is expected to be a file path, and the
    /// pattern will be replaced with the contents of the target file. This allows insertion of
    /// some big JSON state or even HTML files as a part of the `index.html` build.
    ///
    /// Trunk will automatically insert the `base`, `wasm` and `js` key/values into this map. In
    /// order for the app to be loaded properly, the patterns `{base}`, `{wasm}` and `{js}` should
    /// be used in `pattern_script` and `pattern_preload`.
    ///
    /// These values can only be provided via config file.
    pub pattern_params: Option<HashMap<String, String>>,
}

/// Config options for the watch system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsWatch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[clap(short, long, parse(from_os_str), value_name = "path")]
    pub watch: Option<Vec<PathBuf>>,
    /// Paths to ignore [default: []]
    #[clap(short, long, parse(from_os_str), value_name = "path")]
    pub ignore: Option<Vec<PathBuf>>,
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsServe {
    /// The address to serve on [default: 127.0.0.1]
    #[clap(long)]
    pub address: Option<IpAddr>,
    /// The port to serve on [default: 8080]
    #[clap(long)]
    pub port: Option<u16>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[clap(long)]
    #[serde(default)]
    pub open: bool,
    /// A URL to which requests will be proxied [default: None]
    #[clap(long = "proxy-backend")]
    #[serde(default, deserialize_with = "deserialize_uri")]
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[clap(long = "proxy-rewrite")]
    #[serde(default)]
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets [default: false]
    #[clap(long = "proxy-ws")]
    #[serde(default)]
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure requests [default: false]
    #[clap(long = "proxy-insecure")]
    #[serde(default)]
    pub proxy_insecure: bool,
    /// Disable auto-reload of the web app [default: false]
    #[clap(long = "no-autoreload")]
    #[serde(default)]
    pub no_autoreload: bool,
    /// Additional headers to send in responses [default: none]
    #[clap(skip)]
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsClean {
    /// The output dir for all final assets [default: dist]
    #[clap(short, long, parse(from_os_str))]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean [default: false]
    #[clap(long)]
    #[serde(default)]
    pub cargo: bool,
}

/// Config options for automatic application downloads.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOptsTools {
    /// Version of `dart-sass` to use.
    pub sass: Option<String>,
    /// Version of `wasm-bindgen` to use.
    pub wasm_bindgen: Option<String>,
    /// Version of `wasm-opt` to use.
    pub wasm_opt: Option<String>,
}

/// Config options for building proxies.
///
/// NOTE WELL: this configuration type is different from the others inasmuch as it is only used
/// when parsing the `Trunk.toml` config file. It is not intended to be configured via CLI or env
/// vars.
#[derive(Clone, Debug, Deserialize)]
pub struct ConfigOptsProxy {
    /// The URL of the backend to which requests are to be proxied.
    #[serde(deserialize_with = "deserialize_uri")]
    pub backend: Uri,
    /// An optional URI prefix which is to be used as the base URI for proxying requests, which
    /// defaults to the URI of the backend.
    ///
    /// When a value is specified, requests received on this URI will have this URI segment
    /// replaced with the URI of the `backend`.
    pub rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets.
    #[serde(default)]
    pub ws: bool,
    /// Configure the proxy to accept insecure certificates.
    #[serde(default)]
    pub insecure: bool,
}

/// Config options for build system hooks.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigOptsHook {
    /// The stage in the build process to execute this hook.
    pub stage: PipelineStage,
    /// The command to run for this hook.
    pub command: String,
    /// Any arguments to pass to the command.
    #[serde(default)]
    pub command_arguments: Vec<String>,
}

/// Deserialize a Uri from a string.
fn deserialize_uri<'de, D, T>(data: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: std::convert::From<Uri>,
{
    let val = String::deserialize(data)?;
    Uri::from_str(val.as_str())
        .map(Into::into)
        .map_err(|err| serde::de::Error::custom(err.to_string()))
}

/// A model of all potential configuration options for the Trunk CLI system.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOpts {
    pub build: Option<ConfigOptsBuild>,
    pub watch: Option<ConfigOptsWatch>,
    pub serve: Option<ConfigOptsServe>,
    pub clean: Option<ConfigOptsClean>,
    pub tools: Option<ConfigOptsTools>,
    pub proxy: Option<Vec<ConfigOptsProxy>>,
    pub hooks: Option<Vec<ConfigOptsHook>>,
}

impl ConfigOpts {
    /// Extract the runtime config for the build system based on all config layers.
    pub fn rtc_build(cli_build: ConfigOptsBuild, config: Option<PathBuf>) -> Result<Arc<RtcBuild>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let build_opts = build_layer.build.unwrap_or_default();
        let tools_opts = build_layer.tools.unwrap_or_default();
        let hooks_opts = build_layer.hooks.unwrap_or_default();
        Ok(Arc::new(RtcBuild::new(
            build_opts, tools_opts, hooks_opts, false,
        )?))
    }

    /// Extract the runtime config for the watch system based on all config layers.
    pub fn rtc_watch(
        cli_build: ConfigOptsBuild,
        cli_watch: ConfigOptsWatch,
        config: Option<PathBuf>,
    ) -> Result<Arc<RtcWatch>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let build_opts = watch_layer.build.unwrap_or_default();
        let watch_opts = watch_layer.watch.unwrap_or_default();
        let tools_opts = watch_layer.tools.unwrap_or_default();
        let hooks_opts = watch_layer.hooks.unwrap_or_default();
        Ok(Arc::new(RtcWatch::new(
            build_opts, watch_opts, tools_opts, hooks_opts, false,
        )?))
    }

    /// Extract the runtime config for the serve system based on all config layers.
    pub fn rtc_serve(
        cli_build: ConfigOptsBuild,
        cli_watch: ConfigOptsWatch,
        cli_serve: ConfigOptsServe,
        config: Option<PathBuf>,
    ) -> Result<Arc<RtcServe>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let serve_layer = Self::cli_opts_layer_serve(cli_serve, watch_layer);
        let build_opts = serve_layer.build.unwrap_or_default();
        let watch_opts = serve_layer.watch.unwrap_or_default();
        let serve_opts = serve_layer.serve.unwrap_or_default();
        let tools_opts = serve_layer.tools.unwrap_or_default();
        let hooks_opts = serve_layer.hooks.unwrap_or_default();
        Ok(Arc::new(RtcServe::new(
            build_opts,
            watch_opts,
            serve_opts,
            tools_opts,
            hooks_opts,
            serve_layer.proxy,
        )?))
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
            no_default_features: cli.no_default_features,
            all_features: cli.all_features,
            features: cli.features,
            filehash: cli.filehash,
            pattern_script: cli.pattern_script,
            pattern_preload: cli.pattern_preload,
            pattern_params: cli.pattern_params,
        };
        let cfg_build = ConfigOpts {
            build: Some(opts),
            watch: None,
            serve: None,
            clean: None,
            tools: None,
            proxy: None,
            hooks: None,
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
            tools: None,
            proxy: None,
            hooks: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn cli_opts_layer_serve(cli: ConfigOptsServe, cfg_base: Self) -> Self {
        let opts = ConfigOptsServe {
            address: cli.address,
            port: cli.port,
            open: cli.open,
            proxy_backend: cli.proxy_backend,
            proxy_rewrite: cli.proxy_rewrite,
            proxy_insecure: cli.proxy_insecure,
            proxy_ws: cli.proxy_ws,
            no_autoreload: cli.no_autoreload,
            headers: cli.headers,
        };
        let cfg = ConfigOpts {
            build: None,
            watch: None,
            serve: Some(opts),
            clean: None,
            tools: None,
            proxy: None,
            hooks: None,
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
            tools: None,
            proxy: None,
            hooks: None,
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
            trunk_toml_path = trunk_toml_path.canonicalize().with_context(|| {
                format!(
                    "error getting canonical path to Trunk config file {:?}",
                    &trunk_toml_path
                )
            })?;
        }
        let cfg_bytes = std::fs::read(&trunk_toml_path).context("error reading config file")?;
        let mut cfg: Self = toml::from_slice(&cfg_bytes)
            .context("error reading config file contents as TOML data")?;
        if let Some(parent) = trunk_toml_path.parent() {
            if let Some(build) = cfg.build.as_mut() {
                if let Some(target) = build.target.as_mut() {
                    if !target.is_absolute() {
                        *target =
                            std::fs::canonicalize(parent.join(&target)).with_context(|| {
                                format!(
                                    "error taking canonical path to [build].target {:?} in {:?}",
                                    target, trunk_toml_path
                                )
                            })?;
                    }
                }
                if let Some(dist) = build.dist.as_mut() {
                    if !dist.is_absolute() {
                        *dist = parent.join(&dist);
                    }
                }
            }
            if let Some(watch) = cfg.watch.as_mut() {
                if let Some(watch_paths) = watch.watch.as_mut() {
                    for path in watch_paths.iter_mut() {
                        if !path.is_absolute() {
                            *path =
                                std::fs::canonicalize(parent.join(&path)).with_context(|| {
                                    format!(
                                        "error taking canonical path to [watch].watch {:?} in {:?}",
                                        path, trunk_toml_path
                                    )
                                })?;
                        }
                    }
                }
                if let Some(ignore_paths) = watch.ignore.as_mut() {
                    for path in ignore_paths.iter_mut() {
                        if !path.is_absolute() {
                            *path =
                                std::fs::canonicalize(parent.join(&path)).with_context(|| {
                                    format!(
                                        "error taking canonical path to [watch].ignore {:?} in \
                                         {:?}",
                                        path, trunk_toml_path
                                    )
                                })?;
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
        Ok(ConfigOpts {
            build: Some(envy::prefixed("TRUNK_BUILD_").from_env()?),
            watch: Some(envy::prefixed("TRUNK_WATCH_").from_env()?),
            serve: Some(envy::prefixed("TRUNK_SERVE_").from_env()?),
            clean: Some(envy::prefixed("TRUNK_CLEAN_").from_env()?),
            tools: Some(envy::prefixed("TRUNK_TOOLS_").from_env()?),
            proxy: None,
            hooks: None,
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
                g.filehash = g.filehash.or(l.filehash);
                // NOTE: this can not be disabled in the cascade.
                if l.release {
                    g.release = true;
                }
                g.pattern_preload = g.pattern_preload.or(l.pattern_preload);
                g.pattern_script = g.pattern_script.or(l.pattern_script);
                g.pattern_params = g.pattern_params.or(l.pattern_params);
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
                g.address = g.address.or(l.address);
                g.port = g.port.or(l.port);
                g.proxy_ws = g.proxy_ws || l.proxy_ws;
                // NOTE: this can not be disabled in the cascade.
                if l.no_autoreload {
                    g.no_autoreload = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.open {
                    g.open = true;
                }
                g.headers.extend(l.headers);
                Some(g)
            }
        };
        greater.tools = match (lesser.tools.take(), greater.tools.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.sass = g.sass.or(l.sass);
                g.wasm_bindgen = g.wasm_bindgen.or(l.wasm_bindgen);
                g.wasm_opt = g.wasm_opt.or(l.wasm_opt);
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
                    g.cargo = true;
                }
                Some(g)
            }
        };
        greater.proxy = match (lesser.proxy.take(), greater.proxy.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(_), Some(g)) => Some(g), // No meshing/merging. Only take the greater value.
        };
        greater.hooks = match (lesser.hooks.take(), greater.hooks.take()) {
            (None, None) => None,
            (Some(val), None) | (None, Some(val)) => Some(val),
            (Some(_), Some(g)) => Some(g), // No meshing/merging. Only take the greater value.
        };
        greater
    }
}
