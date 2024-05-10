use anyhow::bail;
use semver::{Version, VersionReq};

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

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    // requires, actual, pass?
    #[case("*", "0.19.0", true)]
    #[case("*", "0.19.0-alpha.1", true)]
    #[case("0.19", "0.19.0", true)]
    #[case("0.19.0", "0.19.0", true)]
    #[case("0.19.0", "0.19.1", true)]
    // this may come unexpected, but for 0.x version, a minor version change is a breaking change
    #[case("0.20.0", "0.19.0", false)]
    #[case("0.19.0-alpha.2", "0.19.0-alpha.1", false)]
    #[case("0.19.0-alpha.2", "0.19.0-alpha.2", true)]
    #[case("0.19.0-alpha.2", "0.19.0-alpha.3", true)]
    #[case("0.19.0-alpha.2", "0.19.0", true)]
    #[case("0.19.0-alpha.2", "0.19.1", true)]
    // this may come unexpected, but for 0.x version, a minor version change is a breaking change
    #[case("0.19.0-alpha.2", "0.20.0", false)]
    #[case("0.19.1", "0.19.0", false)]
    #[case("0.19.1", "0.19.0-alpha.1", false)]
    #[case("0.19.1", "0.19.1-alpha.1", false)]
    #[case("0.20.0", "0.19.0-alpha.1", false)]
    #[case("0.20.0", "0.19.0", false)]
    #[case("0.20.0", "0.19.1-alpha.1", false)]
    #[case("0.20.0", "0.19.1", false)]
    // a way to say: 0.19.0 or greater
    #[case(">=0.19.0", "0.19.0", true)]
    #[case(">=0.19.0", "0.19.1", true)]
    #[case(">=0.19.0", "0.20.0", true)]
    // a way to say: 0.19.0-alpha.2 or greater
    #[case(">=0.19.0-alpha.2", "0.19.0-alpha.1", false)]
    #[case(">=0.19.0-alpha.2", "0.19.0-alpha.2", true)]
    #[case(">=0.19.0-alpha.2", "0.19.0-rc.1", true)]
    #[case(">=0.19.0-alpha.2", "0.19.0", true)]
    // The following case comes unexpected
    #[case(">=0.19.0-alpha.2", "0.20.0-alpha.1", false)]
    #[case(">=0.19.0-alpha.2", "0.20.0", true)]
    fn test_requires(
        #[case] required: VersionReq,
        #[case] actual: Version,
        #[case] expected: bool,
    ) {
        assert_eq!(expected, enforce_version_with(&required, actual).is_ok());
    }
}
