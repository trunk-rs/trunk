use super::{NAME, VERSION};
use crate::version::state::Versions;
use semver::Version;
use std::time::Duration;

pub async fn most_recent() -> anyhow::Result<Versions> {
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
