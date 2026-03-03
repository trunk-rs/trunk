use crate::config::models::ConfigModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Config options for build system node module.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct NodePackage {
    /// Optional npm registry to use (default is https://registry.npmjs.org)
    #[serde(default)]
    pub registry: Option<String>,
    /// Package name in https://npmjs.com
    pub name: String,
    /// Version of the package
    pub version: String,
    /// Path where to install the package
    #[serde(default)]
    pub target_path: Option<String>,
}

/// New type for handling `Vec<NodeModule>`
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct NodePackages(
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub Vec<NodePackage>,
);

impl ConfigModel for NodePackages {}
