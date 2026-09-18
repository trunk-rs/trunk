use crate::version::{
    NAME, USER_AGENT, VERSION,
    enabled::state::{State, Versions},
};
use semver::Version;
use serde::Deserialize;
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

    let response = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(format!("https://crates.io/api/v1/crates/{NAME}"))
        .send()
        .await?
        .error_for_status()?
        .json::<CrateResponse>()
        .await?;

    Ok(versions_from_response(response))
}

#[derive(Debug, Deserialize)]
struct CrateResponse {
    versions: Vec<CrateVersion>,
}

#[derive(Debug, Deserialize)]
struct CrateVersion {
    num: String,
    yanked: bool,
}

fn versions_from_response(response: CrateResponse) -> Versions {
    let versions = response
        .versions
        .into_iter()
        .filter(|v| !v.yanked)
        .map(|v| v.num)
        .filter_map(|v| Version::parse(&v).ok())
        .collect::<Vec<_>>();

    let release = versions.iter().filter(|v| v.pre.is_empty()).max().cloned();
    let prerelease = versions.iter().max().cloned();

    Versions {
        release,
        prerelease,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_latest_non_yanked_release_and_prerelease() {
        let Ok(response) = serde_json::from_str::<CrateResponse>(
            r#"{
                "versions": [
                    { "num": "1.2.0", "yanked": false },
                    { "num": "2.0.0-beta.1", "yanked": false },
                    { "num": "1.3.0", "yanked": true },
                    { "num": "not-a-version", "yanked": false }
                ]
            }"#,
        ) else {
            panic!("fixture must be valid");
        };

        let versions = versions_from_response(response);

        assert_eq!(versions.release, Some(Version::new(1, 2, 0)));
        assert_eq!(
            versions.prerelease.as_ref().map(ToString::to_string),
            Some("2.0.0-beta.1".to_owned())
        );
    }
}
