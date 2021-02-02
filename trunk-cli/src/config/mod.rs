//! Trunk config.
//!
//! Trunk takes the typical layered configuration approach. There are 3 layers. The
//! `Trunk.toml` config file is the base, which is then superseded by environment variables,
//! which are finally superseded by CLI arguments and options.

mod manifest;
mod models;
mod rt;

pub use manifest::CargoMetadata;
pub use models::{ConfigOpts, ConfigOptsBuild, ConfigOptsClean, ConfigOptsProxy, ConfigOptsServe, ConfigOptsWatch};
pub use rt::{RtcBuild, RtcClean, RtcServe, RtcWatch};
