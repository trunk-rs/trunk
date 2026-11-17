use crate::config::models::ConfigModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Trunk asset links provided by this package.
    ///
    /// Each entry is injected into the `<head>` of the HTML document as a
    /// `<link data-trunk rel="..." href="..." ...>` element. The `href` is relative to the
    /// directory where the package is installed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<NodePackageLink>,
}

impl NodePackage {
    /// The directory (relative to the working directory) where the package gets installed.
    pub fn install_path(&self) -> String {
        self.target_path
            .clone()
            .unwrap_or_else(|| format!("target/node_modules/{}/{}", self.name, self.version))
    }

    /// Check if both entries refer to the same installation of a package.
    pub fn same_installation(&self, other: &Self) -> bool {
        self.registry == other.registry
            && self.name == other.name
            && self.version == other.version
            && self.install_path() == other.install_path()
    }
}

/// A trunk asset link, provided by a node package.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct NodePackageLink {
    /// The asset type (e.g. `sass`, `css`, `copy-dir`, `copy-file`)
    pub rel: String,
    /// The path to the asset, relative to the package installation directory
    pub href: String,
    /// Additional attributes of the link (e.g. `data-target-path`)
    #[serde(flatten)]
    pub attributes: BTreeMap<String, String>,
}

/// New type for handling `Vec<NodeModule>`
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct NodePackages(
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub Vec<NodePackage>,
);

impl ConfigModel for NodePackages {}
