use crate::config::models::ConfigModel;
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;

/// Config options for the watch system.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Watch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watch: Vec<PathBuf>,

    /// Paths to ignore [default: []]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<PathBuf>,
}

impl ConfigModel for Watch {}
