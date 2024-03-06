mod check;
mod enforce;
mod state;

#[cfg(test)]
pub(crate) use enforce::enforce_version_with;

pub(crate) use enforce::enforce_version;

use crate::common::UPDATE;
use crate::version::state::{State, Versions};
use semver::Version;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(feature = "update_check")]
pub fn update_check(skip: bool) {
    if skip {
        return;
    }

    tracing::debug!("Spawning update check");

    // We need to spawn this in a dedicated tokio runtime, as otherwise this would block
    // the current tokio runtime from existing. There seems to be an issue with where even
    // with an aborted spawned task, tokio will wait for it to end indefinitely.
    std::thread::spawn(|| {
        perform_update_check();
    });
}

#[cfg(not(feature = "update_check"))]
pub fn update_check(_: bool) {}

/// Check if there's a newer version available
#[tokio::main]
async fn perform_update_check() {
    tracing::debug!("Performing update check");

    let versions = match state::need_check().await {
        State::NotNeeded(versions) => {
            tracing::debug!("No refresh needed");
            versions
        }
        State::Needed => match check::most_recent().await {
            Err(err) => {
                tracing::debug!("Failed to check for new version: {err}");
                return;
            }
            Ok(versions) => {
                tracing::debug!("New versions: {versions:?}");
                state::record_checked(versions.clone()).await;
                versions
            }
        },
    };

    announce_version(&versions);
}

/// Announce a new version if it is newer than our current
fn announce_version(versions: &Versions) {
    let Ok(current) = Version::parse(VERSION) else {
        tracing::debug!("Failed to parse current version ({VERSION})");
        return;
    };

    let most_recent = match current.pre.is_empty() {
        false => &versions.prerelease,
        true => &versions.release,
    };

    let Some(most_recent) = most_recent else {
        return;
    };

    tracing::debug!("Current: {current}, Most recent: {most_recent}");

    if most_recent > &current {
        tracing::info!("{UPDATE}Found an update of {NAME}: {VERSION} -> {most_recent}");
    }
}
