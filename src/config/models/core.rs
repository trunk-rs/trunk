use semver::VersionReq;
use serde::Deserialize;

/// Config options for the core project.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOptsCore {
    #[serde(default)]
    // align that with cargo's `rust-version`
    #[serde(alias = "trunk-version")]
    pub trunk_version: Option<VersionReq>,
}
