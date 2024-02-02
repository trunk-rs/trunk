use super::super::DIST_DIR;
use crate::config::{ConfigOptsClean, ConfigOptsCore, RtcCore};
use std::path::PathBuf;
use std::sync::Arc;

/// Runtime config for the clean system.
#[derive(Clone, Debug)]
pub struct RtcClean {
    pub core: Arc<RtcCore>,
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
}

impl RtcClean {
    pub(crate) fn new(core: ConfigOptsCore, opts: ConfigOptsClean) -> Self {
        let core = Arc::new(RtcCore::new(core));

        Self {
            core,
            dist: opts.dist.unwrap_or_else(|| DIST_DIR.into()),
            cargo: opts.cargo,
        }
    }
}
