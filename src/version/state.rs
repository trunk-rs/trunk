use semver::Version;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

const CHECK_PERIOD: Duration = Duration::days(1);

/// Get the path to the state file.
fn state_file() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    let path = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());

    Some(path.join(env!("CARGO_PKG_NAME")).join("update.json"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StateInformation {
    #[serde(with = "time::serde::rfc3339")]
    pub last_check: OffsetDateTime,

    pub versions: Versions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    /// The most recent released version
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<Version>,

    /// The most recent version, including pre-releases
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<Version>,
}

#[derive(Clone, Debug)]
pub enum State {
    NotNeeded(Versions),
    Needed,
}

/// Evaluate if we are due for a check
///
/// Returns `false` if anything goes wrong.
pub async fn need_check() -> State {
    let Some(file) = state_file() else {
        tracing::debug!("Unable to find a user home. Skipping update checks.");
        return State::NotNeeded(Default::default());
    };

    let state = match tokio::fs::read(&file).await {
        Err(err) if err.kind() == ErrorKind::NotFound => return State::Needed,
        Err(err) => {
            tracing::debug!(
                "Failed to check update state file ({}), skipping: {err}",
                file.display()
            );
            return State::NotNeeded(Default::default());
        }
        Ok(state) => state,
    };

    let Ok(state) = serde_json::from_slice::<StateInformation>(&state) else {
        // if we can't read the file, check and re-write
        return State::Needed;
    };

    let diff = OffsetDateTime::now_utc() - state.last_check;

    tracing::debug!("Time since last check: {diff}");

    if diff > CHECK_PERIOD {
        State::Needed
    } else {
        State::NotNeeded(state.versions)
    }
}

/// Record that we did perform a check.
///
/// Silently ignores errors.
pub async fn record_checked(versions: Versions) {
    let Some(file) = state_file() else {
        tracing::debug!("Unable to find a user home. Skipping update checks.");
        return;
    };

    let state = match serde_json::to_vec(&StateInformation {
        last_check: OffsetDateTime::now_utc(),
        versions,
    }) {
        Ok(state) => state,
        Err(err) => {
            tracing::debug!("Unable to serialize state file: {err}");
            return;
        }
    };

    if let Some(parent) = file.parent() {
        if let Err(err) = tokio::fs::create_dir_all(parent).await {
            tracing::debug!(
                "Failed to create parent directory for update state ({}): {err}",
                parent.display()
            );
            return;
        }
    }

    if let Err(err) = tokio::fs::write(&file, state).await {
        tracing::debug!(
            "Failed to write update state file ({}): {err}",
            file.display()
        );
    }
}
