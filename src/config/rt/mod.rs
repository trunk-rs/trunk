mod build;
mod clean;
mod serve;
mod watch;

pub use build::*;
pub use clean::*;
pub use serve::*;
pub use watch::*;

use crate::config::models::ConfigOptsCore;
use semver::VersionReq;

/// Runtime config for the core project.
#[derive(Clone, Debug)]
pub struct RtcCore {
    pub trunk_version: VersionReq,
}

impl RtcCore {
    pub(super) fn new(opts: ConfigOptsCore) -> Self {
        let ConfigOptsCore { trunk_version } = opts;
        Self {
            trunk_version: trunk_version.unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(super) fn new_test() -> Self {
        RtcCore {
            trunk_version: VersionReq::STAR,
        }
    }
}
