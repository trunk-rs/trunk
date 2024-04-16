use crate::config::{RtcBuild, RtcClean, RtcServe, RtcWatch};
use anyhow::{Context, Result};
use axum::http::Uri;
use serde::{Deserialize, Deserializer};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

#[cfg(test)]
mod test;

mod build;
mod clean;
mod core;
mod hook;
mod proxy;
mod serve;
mod tools;
mod types;
mod watch;

pub use build::*;
pub use clean::*;
pub use core::*;
pub use hook::*;
pub use proxy::*;
pub use serve::*;
pub use tools::*;
pub use types::*;
pub use watch::*;

/// Deserialize a Uri from a string.
fn deserialize_uri<'de, D, T>(data: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: From<Uri>,
{
    let val = String::deserialize(data)?;
    Uri::from_str(val.as_str())
        .map(Into::into)
        .map_err(|err| serde::de::Error::custom(err.to_string()))
}

/// A model of all potential configuration options for the Trunk CLI system.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOpts {
    #[serde(flatten)]
    pub core: Option<ConfigOptsCore>,

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
        let core_opts = base_layer.core.clone().unwrap_or_default();
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let build_opts = build_layer.build.unwrap_or_default();
        let tools_opts = build_layer.tools.unwrap_or_default();
        let hooks_opts = build_layer.hooks.unwrap_or_default();
        Ok(Arc::new(RtcBuild::new(
            core_opts, build_opts, tools_opts, hooks_opts, false,
        )?))
    }

    /// Extract the runtime config for the watch system based on all config layers.
    pub fn rtc_watch(
        cli_build: ConfigOptsBuild,
        cli_watch: ConfigOptsWatch,
        config: Option<PathBuf>,
    ) -> Result<Arc<RtcWatch>> {
        let base_layer = Self::file_and_env_layers(config)?;
        let core_opts = base_layer.core.clone().unwrap_or_default();
        let build_layer = Self::cli_opts_layer_build(cli_build, base_layer);
        let watch_layer = Self::cli_opts_layer_watch(cli_watch, build_layer);
        let build_opts = watch_layer.build.unwrap_or_default();
        let watch_opts = watch_layer.watch.unwrap_or_default();
        let tools_opts = watch_layer.tools.unwrap_or_default();
        let hooks_opts = watch_layer.hooks.unwrap_or_default();
        Ok(Arc::new(RtcWatch::new(
            core_opts, build_opts, watch_opts, tools_opts, hooks_opts, false, false,
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
        let core_opts = base_layer.core.clone().unwrap_or_default();
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
                core_opts,
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
        let core_opts = base_layer.core.clone().unwrap_or_default();
        let clean_layer = Self::cli_opts_layer_clean(cli_clean, base_layer);
        let clean_opts = clean_layer.clean.unwrap_or_default();
        Ok(Arc::new(RtcClean::new(core_opts, clean_opts)))
    }

    /// Return the full configuration based on config file & environment variables.
    pub fn full(config: Option<PathBuf>) -> Result<Self> {
        Self::file_and_env_layers(config)
    }

    fn cli_opts_layer_build(cli: ConfigOptsBuild, cfg_base: Self) -> Self {
        let cfg_build = ConfigOpts {
            core: None,
            build: Some(cli),
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
        let cfg = ConfigOpts {
            core: None,
            build: None,
            watch: Some(cli),
            serve: None,
            clean: None,
            tools: None,
            proxy: None,
            hooks: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn cli_opts_layer_serve(cli: ConfigOptsServe, cfg_base: Self) -> Self {
        let cfg = ConfigOpts {
            core: None,
            build: None,
            watch: None,
            serve: Some(cli),
            clean: None,
            tools: None,
            proxy: None,
            hooks: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn cli_opts_layer_clean(cli: ConfigOptsClean, cfg_base: Self) -> Self {
        let cfg = ConfigOpts {
            core: None,
            build: None,
            watch: None,
            serve: None,
            clean: Some(cli),
            tools: None,
            proxy: None,
            hooks: None,
        };
        Self::merge(cfg_base, cfg)
    }

    fn file_and_env_layers(path: Option<PathBuf>) -> Result<Self> {
        let toml_cfg = Self::from_file(path.clone())?;
        let env_cfg = Self::from_env().context("error reading trunk env var config")?;
        let mut cfg = Self::merge(toml_cfg, env_cfg);

        // We always set the working directory with the parent of the configuration. So that
        // we have a canonical location of the expected working directory.

        let core = cfg.core.get_or_insert(ConfigOptsCore::default());
        core.working_directory = path.and_then(|path| path.parent().map(|p| p.to_path_buf()));

        // return the result

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
            core: Some(ConfigOptsCore::from_env()?),
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
        greater.core = match (lesser.core.take(), greater.core.take()) {
            (None, None) => None,
            (Some(val), None) => Some(val),
            (None, Some(val)) => Some(val),
            (Some(l), Some(mut g)) => {
                g.trunk_version = g.trunk_version.or(l.trunk_version);

                Some(g)
            }
        };

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
                // NOTE: this can not be disabled in the cascade.
                if l.no_minification {
                    g.no_minification = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.no_sri {
                    g.no_sri = true;
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
                // for the address/addreses, we override both or none
                match (g.address.is_some(), g.addresses.is_some()) {
                    (true, _) | (_, true) => {
                        g.addresses =
                            Some(g.addresses.into_iter().flatten().chain(g.address).collect());
                        g.address = None;
                    }
                    _ => {
                        g.addresses =
                            Some(l.addresses.into_iter().flatten().chain(l.address).collect());
                        g.address = None;
                    }
                }
                g.port = g.port.or(l.port);
                g.proxy_ws = g.proxy_ws || l.proxy_ws;
                g.ws_protocol = g.ws_protocol.or(l.ws_protocol);
                g.tls_key_path = g.tls_key_path.or(l.tls_key_path);
                g.tls_cert_path = g.tls_cert_path.or(l.tls_cert_path);
                g.serve_base = g.serve_base.or(l.serve_base);
                g.ws_base = g.ws_base.or(l.ws_base);
                // NOTE: this can not be disabled in the cascade.
                if l.no_autoreload {
                    g.no_autoreload = true;
                }
                // NOTE: this can not be disabled in the cascade.
                if l.no_spa {
                    g.no_spa = true;
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
