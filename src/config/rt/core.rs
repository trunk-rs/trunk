use crate::config::ConfigOptsCore;
use semver::VersionReq;
use std::path::PathBuf;

/// Runtime config for the core project.
#[derive(Clone, Debug)]
pub struct RtcCore {
    pub trunk_version: VersionReq,
    pub working_directory: PathBuf,
}

impl RtcCore {
    pub(super) fn new(opts: ConfigOptsCore) -> Self {
        let ConfigOptsCore {
            trunk_version,
            working_directory,
        } = opts;
        Self {
            trunk_version: trunk_version.unwrap_or_default(),
            working_directory: working_directory
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(super) fn new_test() -> Self {
        RtcCore {
            trunk_version: VersionReq::STAR,
            working_directory: Default::default(),
        }
    }
}
