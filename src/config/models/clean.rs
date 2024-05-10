//! Configuration for "clean"
use crate::config::models::ConfigModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Config options for the serve system.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct Clean {
    /// The output dir for all final assets
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[deprecated(note = "Use the global dist field instead")]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean
    #[serde(default)]
    pub cargo: bool,
}

impl ConfigModel for Clean {}
