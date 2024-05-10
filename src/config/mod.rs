//! Trunk config.
//!
//! Trunk follows a layered configuration approach. There are reasonable defaults, the option
//! to load from a configuration file, followed by overrides from the command line (or env-vars).
//!
//! There are four types of structs: Command Line, Serialization, Runtime Options, and Runtime.
//!
//! ## Command Line
//!
//! The command line structs are based on [`clap`] and support both arguments and environment
//! variables. The command line structs can be found in their respective [`crate::cmd`] module.
//!
//! Most fields in those structs are optional, as the idea is to override what can be found in
//! the configuration.
//!
//! Some options in the command line structs are more intended to influence "what" to run, rather
//! than "how" to execute a command. If that's the case, then this option would not be part of
//! the configuration coming from the configuration files.
//!
//! ## Serialization
//!
//! Trunk has a "project model", covering the aspects of what make up a Trunk project. This model
//! is based on structs in the [`crate::config::models`] module and based on [`serde`]. It is
//! loaded in from a configuration file (TOML, YAML, …) or even the `Cargo.toml` file's metadata.
//!
//! However, there is no hierarchical loading process. The first source found will be used.
//!
//! These structs do have a lot of non-optional fields with defaults. These structs should define
//! the defaults to use if they are not provided in the configuration by the user.
//!
//! ## Runtime
//!
//! The runtime configuration structs in [`crate::config::rt`] contain all the information a Trunk
//! command requires to execute, in the form the command requires it.
//!
//! ## Runtime Options
//!
//! Runtime options are a bit of an outlier. In some cases, the runtime configuration requires some
//! information that doesn't come from the user, but from the command execute it. For example,
//! the "build" process needs to know if the reload websocket needs to be injected. Such information
//! is passed in via the Runtime Options.
//!
//! In some cases, it might also be possible that for some commands the command decides, but in
//! other cases the user might influence the choice. This can also go into those options (see
//! error reporting).
//!
//! ## Bringing it all together
//!
//! Here is how it works in general:
//!
//! * Trunk parses the command line arguments (including env-vars) via `clap`
//! * Trunk finds a configuration source (taking the `--config` argument under consideration).
//! * The configuration file is loaded via `serde` and/or `cargo_metadata`.
//! * The configuration model is validated, this might trigger warnings about deprecated fields.
//! * The command line arguments get applied to the configuration, overriding the configuration.
//! * The command creates the runtime options required for the command.
//! * Trunk creates the runtime configuration from the configuration and the runtime options.
//!
//! One aspect of this is that Trunk will always validate the full configuration, but will only
//! create the required runtime configuration for the requested command.

pub mod manifest;
pub mod models;
pub mod rt;
pub mod types;

/// The default name of the directory where final build artifacts are
/// placed after a successful build.
pub const DIST_DIR: &str = "dist";
/// The name of the directory used to stage build artifacts during an active build.
pub const STAGE_DIR: &str = ".stage";

pub use manifest::CargoMetadata;
pub use models::{load, Clean, Configuration, Hooks, Tools, Watch};
