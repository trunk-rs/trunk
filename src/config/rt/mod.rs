//! The runtime configuration
//!
//! This is what the system actually uses.

mod build;
mod clean;
mod core;
mod serve;
mod watch;

pub use build::*;
pub use clean::*;
pub use core::*;
pub use serve::*;
pub use watch::*;

use crate::config::Configuration;
use std::path::PathBuf;

/// Build a runtime configuration from configuration and options.
pub trait RtcBuilder: Sized {
    type Options: Sized;

    async fn build(configuration: Configuration, options: Self::Options) -> anyhow::Result<Self>;

    async fn from_config<F>(
        configuration: Configuration,
        working_directory: PathBuf,
        f: F,
    ) -> anyhow::Result<Self>
    where
        F: FnOnce(&Configuration, CoreOptions) -> Self::Options,
    {
        let opts = f(&configuration, CoreOptions { working_directory });
        Self::build(configuration, opts).await
    }
}
