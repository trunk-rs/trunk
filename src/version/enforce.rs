use crate::config::RtcCore;
use crate::version::VERSION;
use anyhow::bail;
use semver::{Version, VersionReq};

/// Ensure that we are the right trunk version for the project
pub(crate) fn enforce_version(core: &RtcCore) -> anyhow::Result<()> {
    let actual = match Version::parse(VERSION) {
        Err(err) => {
            tracing::warn!("Unable to parse trunk version, skipping version check: {err}");
            return Ok(());
        }
        Ok(version) => version,
    };

    enforce_version_with(&core.trunk_version, actual)
}

/// Ensure that we are the right trunk version for the project
pub(crate) fn enforce_version_with(required: &VersionReq, actual: Version) -> anyhow::Result<()> {
    tracing::debug!("Enforce version - actual: {actual}, required: {required}");

    if required == &VersionReq::STAR {
        // this should match, but does not match any pre-release version. Which we still accept in this case.
        return Ok(());
    }

    let outcome = required.matches(&actual);
    tracing::debug!("Current version: {actual}, required version: {required}, matches: {outcome}");

    if !outcome {
        bail!("Project requires a trunk version of '{required}', the current trunk version is: '{actual}'");
    }

    Ok(())
}
