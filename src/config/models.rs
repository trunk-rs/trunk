use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::http::Uri;
use clap::{Args, ValueEnum};
use humantime_serde::re::humantime;
use serde::{Deserialize, Deserializer};

use crate::common::parse_public_url;
use crate::config::{RtcBuild, RtcClean, RtcServe, RtcWatch};
use crate::pipelines::PipelineStage;

/// Config options for the build system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsBuild {
    /// The index HTML file to drive the bundling process [default: index.html]
    pub target: Option<PathBuf>,

    /// Build in release mode [default: false]
    #[arg(long)]
    #[serde(default)]
    pub release: bool,

    /// The output dir for all final assets [default: dist]
    #[arg(short, long)]
    pub dist: Option<PathBuf>,

    /// Run without accessing the network
    #[arg(long)]
    #[serde(default)]
    pub offline: bool,

    /// Require Cargo.lock and cache are up to date
    #[arg(long)]
    #[serde(default)]
    pub frozen: bool,

    /// Require Cargo.lock is up to date
    #[arg(long)]
    #[serde(default)]
    pub locked: bool,

    /// Build without downloading required tools [default: false]
    #[arg(long, value_parser = parse_public_url)]
    pub public_url: Option<String>,

    /// Build without default features [default: false]
    #[arg(long)]
    #[serde(default)]
    pub no_default_features: bool,

    /// Build with all features [default: false]
    #[arg(long)]
    #[serde(default)]
    pub all_features: bool,

    /// A comma-separated list of features to activate, must not be used with all-features
    /// [default: ""]
    #[arg(long)]
    pub features: Option<String>,

    /// Whether to include hash values in the output file names [default: true]
    #[arg(long)]
    pub filehash: Option<bool>,

    /// Optional pattern for the app loader script [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly load the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub pattern_script: Option<String>,

    /// Whether to inject scripts into your index file. [default: true]
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub inject_scripts: Option<bool>,

    /// Optional pattern for the app preload element [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly preload the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub pattern_preload: Option<String>,

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
    #[arg(skip)]
    #[serde(default)]
    pub pattern_params: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug)]
pub struct ConfigDuration(pub Duration);

impl<'de> Deserialize<'de> for ConfigDuration {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(humantime_serde::deserialize(deserializer)?))
    }
}

impl FromStr for ConfigDuration {
    type Err = humantime::DurationError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(humantime::Duration::from_str(s)?.into()))
    }
}

/// Config options for the watch system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsWatch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[arg(short, long, value_name = "path")]
    pub watch: Option<Vec<PathBuf>>,
    /// Paths to ignore [default: []]
    #[arg(short, long, value_name = "path")]
    pub ignore: Option<Vec<PathBuf>>,
    /// Using polling mode for detecting changes
    #[arg(long)]
    #[serde(default)]
    pub poll: bool,
    /// The polling interval, when polling is enabled
    #[arg(long)]
    #[serde(default)]
    pub poll_interval: Option<ConfigDuration>,
    /// Allow enabling a cooldown, discarding all change events during the build [default: false]
    #[arg(long)]
    #[serde(default)]
    pub enable_cooldown: bool,
}

/// WebSocket protocol
#[derive(Clone, Copy, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum WsProtocol {
    Wss,
    Ws,
}

impl Display for WsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WsProtocol::Wss => "wss",
                WsProtocol::Ws => "ws",
            }
        )
    }
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsServe {
    /// The address to serve on [default: 127.0.0.1]
    #[arg(long)]
    pub address: Option<IpAddr>,
    /// The port to serve on [default: 8080]
    #[arg(long)]
    pub port: Option<u16>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[arg(long)]
    #[serde(default)]
    pub open: bool,
    /// A URL to which requests will be proxied [default: None]
    #[arg(long = "proxy-backend")]
    #[serde(default, deserialize_with = "deserialize_uri")]
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[arg(long = "proxy-rewrite")]
    #[serde(default)]
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets [default: false]
    #[arg(long = "proxy-ws")]
    #[serde(default)]
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure requests [default: false]
    #[arg(long = "proxy-insecure")]
    #[serde(default)]
    pub proxy_insecure: bool,
    /// Disable auto-reload of the web app [default: false]
    #[arg(long = "no-autoreload")]
    #[serde(default)]
    pub no_autoreload: bool,
    /// Additional headers to send in responses [default: none]
    #[clap(skip)]
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Disable error reporting in the browser [default: false]
    #[arg(long = "no-error-reporting")]
    #[serde(default)]
    pub no_error_reporting: bool,
    /// Protocol used for the auto-reload WebSockets connection [enum: ws, wss]
    #[arg(long = "ws-protocol")]
    pub ws_protocol: Option<WsProtocol>,
    /// The TLS key file to enable TLS encryption [default: None]
    #[arg(long)]
    pub tls_key_path: Option<PathBuf>,
    /// The TLS cert file to enable TLS encryption [default: None]
    #[arg(long)]
    pub tls_cert_path: Option<PathBuf>,
}

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct ConfigOptsClean {
    /// The output dir for all final assets [default: dist]
    #[arg(short, long)]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean [default: false]
    #[arg(long)]
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
    /// Version of `tailwindcss-cli` to use.
    pub tailwindcss: Option<String>,
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
            build_opts, watch_opts, tools_opts, hooks_opts, false, false,
        )?))
    }

    /// Extract the runtime config for the serve system based on all config layers.
    pub async fn rtc_serve(
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
        Ok(Arc::new(
            RtcServe::new(
                build_opts,
                watch_opts,
                serve_opts,
                tools_opts,
                hooks_opts,
                serve_layer.proxy,
            )
            .await?,
        ))
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
            inject_scripts: cli.inject_scripts,
            pattern_script: cli.pattern_script,
            pattern_preload: cli.pattern_preload,
            pattern_params: cli.pattern_params,
            offline: cli.offline,
            frozen: cli.frozen,
            locked: cli.locked,
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
            poll: cli.poll,
            poll_interval: cli.poll_interval,
            enable_cooldown: cli.enable_cooldown,
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
            no_error_reporting: cli.no_error_reporting,
            ws_protocol: cli.ws_protocol,
            tls_key_path: cli.tls_key_path,
            tls_cert_path: cli.tls_cert_path,
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
        let cfg_bytes =
            std::fs::read_to_string(&trunk_toml_path).context("error reading config file")?;
        let mut cfg: Self = toml::from_str(&cfg_bytes)
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
            if let Some(serve) = cfg.serve.as_mut() {
                if let Some(tls_key_path) = serve.tls_key_path.as_mut() {
                    if !tls_key_path.is_absolute() {
                        *tls_key_path = parent.join(&tls_key_path);
                    }
                }
                if let Some(tls_cert_path) = serve.tls_cert_path.as_mut() {
                    if !tls_cert_path.is_absolute() {
                        *tls_cert_path = parent.join(&tls_cert_path);
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
                // NOTE: this can not be disabled in the cascade.
                if l.offline {
                    g.offline = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.frozen {
                    g.frozen = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.locked {
                    g.locked = true;
                }
                g.inject_scripts = g.inject_scripts.or(l.inject_scripts);
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
                g.ws_protocol = g.ws_protocol.or(l.ws_protocol);
                g.tls_key_path = g.tls_key_path.or(l.tls_key_path);
                g.tls_cert_path = g.tls_cert_path.or(l.tls_cert_path);
                // NOTE: this can not be disabled in the cascade.
                if l.no_autoreload {
                    g.no_autoreload = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.open {
                    g.open = true;
                }
                g.headers.extend(l.headers);
                // NOTE: this can not be disabled in the cascade.
                if l.no_error_reporting {
                    g.no_error_reporting = true;
                }
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
                g.tailwindcss = g.tailwindcss.or(l.tailwindcss);
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

/// Cross origin setting
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum CrossOrigin {
    #[default]
    Anonymous,
    UseCredentials,
}

impl CrossOrigin {
    pub fn from_str(s: &str) -> Result<Self, CrossOriginParseError> {
        Ok(match s {
            "" | "anonymous" => CrossOrigin::Anonymous,
            "use-credentials" => CrossOrigin::UseCredentials,
            _ => return Err(CrossOriginParseError::InvalidValue),
        })
    }
}

impl Display for CrossOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anonymous => write!(f, "anonymous"),
            Self::UseCredentials => write!(f, "use-credentials"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CrossOriginParseError {
    #[error("invalid value")]
    InvalidValue,
}

/// Integrity type for subresource protection
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Integrity {
    None,
    Sha256,
    #[default]
    Sha384,
    Sha512,
}

impl FromStr for Integrity {
    type Err = IntegrityParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "" => Default::default(),
            "none" => Self::None,
            "sha256" => Self::Sha256,
            "sha384" => Self::Sha384,
            "sha512" => Self::Sha512,
            _ => return Err(IntegrityParseError::InvalidValue),
        })
    }
}

impl Display for Integrity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Sha256 => write!(f, "sha256"),
            Self::Sha384 => write!(f, "sha384"),
            Self::Sha512 => write!(f, "sha512"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrityParseError {
    #[error("invalid value")]
    InvalidValue,
}
