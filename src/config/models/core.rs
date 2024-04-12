use semver::VersionReq;
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

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

impl ConfigOptsCore {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            trunk_version: std::env::var("TRUNK_REQUIRED_VERSION")
                .ok()
                .map(|value| VersionReq::from_str(&value))
                .transpose()?,
            // the working directory cannot be overridden this way
            working_directory: None,
        })
    }
}
