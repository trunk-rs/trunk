use semver::VersionReq;
use serde::Deserialize;
use std::path::PathBuf;

/// Config options for the core project.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOptsCore {
    #[serde(default)]
    // align that with cargo's `rust-version`
    #[serde(alias = "trunk-version")]
    pub trunk_version: Option<VersionReq>,
    #[serde(skip)]
    pub working_directory: Option<PathBuf>,
}
