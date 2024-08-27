//! The configuration model
//!
//! This is what the user provides, and which gets converted into the runtime model. The CLI will
//! override certain aspects of it when running commands.

pub mod source;

mod build;
mod clean;
mod core;
mod hook;
mod proxy;
mod serve;
mod tools;
mod watch;

pub use build::*;
pub use clean::*;
pub use core::*;
pub use hook::*;
pub use proxy::*;
pub use serve::*;
pub use tools::*;
pub use watch::*;

#[cfg(test)]
mod test;

use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use source::Source;
use std::path::PathBuf;
use tracing::log;

/// Common configuration model functionality
pub trait ConfigModel {
    /// Migrate legacy constructs to newer ones, if possible
    fn migrate(&mut self) -> Result<()> {
        Ok(())
    }
}

/// The persisted Trunk configuration model
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Configuration {
    #[serde(flatten)]
    pub core: Core,

    #[serde(default)]
    pub build: Build,

    #[serde(default)]
    pub tools: Tools,

    #[serde(default)]
    pub hooks: Hooks,

    #[serde(default)]
    pub watch: Watch,

    #[serde(default)]
    pub serve: Serve,

    #[serde(default)]
    pub clean: Clean,

    #[serde(default)]
    #[serde(alias = "proxy")]
    pub proxies: Proxies,
}

impl ConfigModel for Configuration {
    /// Run all migration steps.
    ///
    /// NOTE: This will work on the current instance only and will not alter any configuration files
    #[allow(deprecated)]
    fn migrate(&mut self) -> Result<()> {
        self.core.migrate()?;

        self.tools.migrate()?;
        self.hooks.migrate()?;
        self.proxies.migrate()?;

        self.clean.migrate()?;
        self.build.migrate()?;
        self.watch.migrate()?;
        self.serve.migrate()?;

        // handle migrations with global impact

        // handle the old `clean.dist` field
        if let Some(dist) = self.clean.dist.take() {
            log::warn!("'clean.dist' is used in the configuration. This is deprecated for the global 'dist' field and will result in an error in a future release.");
            self.core.dist = Some(dist);
        }

        // handle single proxy setting

        if let Some(backend) = self.serve.proxy_backend.take() {
            log::warn!("The proxy fields in the configuration are deprecated and will be removed in a future version. Migrate those settings into an entry of the `proxies` field, which allows adding more than one.");
            self.proxies.0.push(Proxy {
                backend,
                request_headers: Default::default(),
                rewrite: self.serve.proxy_rewrite.take(),
                ws: self.serve.proxy_ws.unwrap_or_default(),
                insecure: self.serve.proxy_insecure.unwrap_or_default(),
                no_system_proxy: self.serve.proxy_no_system_proxy.unwrap_or_default(),
                no_redirect: self.serve.proxy_no_redirect.unwrap_or_default(),
            })
        }

        Ok(())
    }
}

/// Locate and load the configuration, given an optional file or directory. Falling back to the
/// current directory.
pub async fn load(path: Option<PathBuf>) -> Result<(Configuration, PathBuf)> {
    match path {
        // if we have a file, load it
        Some(path) if path.is_file() => {
            // Canonicalize the path to the configuration, so that we get a proper parent.
            // Otherwise, we might end up with a parent of '', which won't work later on.
            let path = path.canonicalize().with_context(|| {
                format!(
                    "unable to canonicalize path to configuration: '{}'",
                    path.display()
                )
            })?;
            let Some(cwd) = path.parent() else {
                bail!("unable to get parent directory of '{}'", path.display());
            };
            let cwd = cwd.to_path_buf();

            Ok((Source::File(path).load().await?, cwd))
        }
        // if we have a directory, try finding a file and load it
        Some(path) if path.is_dir() => Ok((Source::find(&path)?.load().await?, path)),
        // if we have something else, we can't deal with it
        Some(path) => bail!("{} is neither a file nor a directory", path.display()),
        // if we have nothing, try to find a file in the current directory and load it
        None => {
            let cwd = std::env::current_dir().context("unable to get current directory")?;
            Ok((Source::find(&cwd)?.load().await?, cwd))
        }
    }
}
