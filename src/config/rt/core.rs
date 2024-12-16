use crate::{config::models::Core, config::DIST_DIR, version::enforce_version_with};
use anyhow::Context;
use semver::{Version, VersionReq};
use std::path::PathBuf;

/// Runtime config for the core project.
#[derive(Clone, Debug)]
pub struct RtcCore {
    pub trunk_version: VersionReq,
    pub working_directory: PathBuf,
    pub dist: PathBuf,
}

#[derive(Clone, Debug)]
pub struct CoreOptions {
    pub working_directory: PathBuf,
}

impl RtcCore {
    pub(super) fn new(config: Core, opts: CoreOptions) -> anyhow::Result<Self> {
        let CoreOptions { working_directory } = opts;

        let trunk_version = config.trunk_version.clone();

        let working_directory = dunce::canonicalize(&working_directory)
            .with_context(|| format!("unable to canonicalize '{}'", working_directory.display()))?;

        let dist =
            working_directory.join(config.dist.as_deref().unwrap_or_else(|| DIST_DIR.as_ref()));

        Ok(Self {
            trunk_version,
            working_directory,
            dist,
        })
    }

    /// Ensure that we are the right trunk version for the project
    pub(crate) fn enforce_version(self: &RtcCore) -> anyhow::Result<()> {
        let actual = match Version::parse(crate::version::VERSION) {
            Err(err) => {
                tracing::warn!("Unable to parse trunk version, skipping version check: {err}");
                return Ok(());
            }
            Ok(version) => version,
        };

        enforce_version_with(&self.trunk_version, actual)
    }

    #[cfg(test)]
    pub(super) fn new_test(root: &std::path::Path) -> Self {
        RtcCore {
            trunk_version: VersionReq::STAR,
            working_directory: root.to_path_buf(),
            dist: root.join(DIST_DIR),
        }
    }
}
