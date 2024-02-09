use super::super::DIST_DIR;
use crate::config::ConfigOptsClean;
use std::path::PathBuf;

/// Runtime config for the clean system.
#[derive(Clone, Debug)]
pub struct RtcClean {
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
}

impl RtcClean {
    pub(crate) fn new(opts: ConfigOptsClean) -> Self {
        Self {
            dist: opts.dist.unwrap_or_else(|| DIST_DIR.into()),
            cargo: opts.cargo,
        }
    }
}
