//! Trunk config.
//!
//! Trunk takes the typical layered configuration approach. There are 3 layers. The
//! `Trunk.toml` config file is the base, which is then superseded by environment variables,
//! which are finally superseded by CLI arguments and options.

mod manifest;
mod models;
#[cfg(test)]
mod models_test;
mod rt;

/// The default name of the directory where final build artifacts are
/// placed after a successful build.
pub const DIST_DIR: &str = "dist";
/// The name of the directory used to stage build artifacts during an active build.
pub const STAGE_DIR: &str = ".stage";

pub use manifest::CargoMetadata;
pub use models::{
    ConfigOpts, ConfigOptsBuild, ConfigOptsClean, ConfigOptsHook, ConfigOptsProxy, ConfigOptsServe,
    ConfigOptsTools, ConfigOptsWatch, CrossOrigin, CrossOriginParseError, WsProtocol,
};
pub use rt::{Features, RtcBuild, RtcClean, RtcServe, RtcWatch};
