use crate::version::{
    enabled::state::{State, Versions},
    NAME, VERSION,
};
use semver::Version;
use std::time::Duration;
use tracing::instrument;

mod state;

#[instrument]
pub fn update_check(skip: bool) {
    tracing::trace!("Update check");

    if skip {
        return;
    }

    tracing::debug!("Spawning update check");

    // We need to spawn this in a dedicated tokio runtime, as otherwise this would block
    // the current tokio runtime from exiting. There seems to be an issue with where even
    // with an aborted spawned task, tokio will wait for it to end indefinitely.
    std::thread::spawn(|| {
        perform_update_check();
    });
}

/// Check if there's a newer version available
#[cfg(feature = "update_check")]
#[tokio::main]
async fn perform_update_check() {
    tracing::debug!("Performing update check");

    let versions = match state::need_check().await {
        State::NotNeeded(versions) => {
            tracing::debug!("No refresh needed");
            versions
        }
        State::Needed => match most_recent().await {
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
#[cfg(feature = "update_check")]
fn announce_version(versions: &Versions) {
    let Ok(current) = Version::parse(VERSION) else {
        tracing::debug!("Failed to parse the current version ({VERSION})");
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
        tracing::info!(
            "{icon}Found an update of {NAME}: {VERSION} -> {most_recent}",
            icon = crate::common::UPDATE
        );
    }
}

async fn most_recent() -> anyhow::Result<Versions> {
    tracing::debug!("Checking for updates");

    let client =
        crates_io_api::AsyncClient::new(&format!("{NAME}/{VERSION}"), Duration::from_secs(1))?;
    let response = client.get_crate(NAME).await?;

    let versions = response
        .versions
        .into_iter()
        .filter(|v| !v.yanked)
        .map(|v| v.num)
        .filter_map(|v| Version::parse(&v).ok())
        .collect::<Vec<_>>();

    let release = versions.iter().filter(|v| v.pre.is_empty()).max().cloned();
    let prerelease = versions.iter().max().cloned();

    Ok(Versions {
        release,
        prerelease,
    })
}
