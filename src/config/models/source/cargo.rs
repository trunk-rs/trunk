//! Loading trunk's configuration from cargo's manifest

use crate::config::{manifest, Configuration};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
struct TrunkMetadata {
    #[serde(rename = "trunk")]
    #[serde(default)]
    pub configuration: Configuration,
}

/// Load the trunk configuration from the cargo manifest
pub async fn from_manifest(file: impl AsRef<Path>) -> anyhow::Result<Configuration> {
    let manifest = manifest::CargoMetadata::new(file.as_ref()).await?;
    let TrunkMetadata { configuration } =
        serde_json::from_value::<Option<_>>(manifest.package.metadata)?.unwrap_or_default();
    Ok(configuration)
}

#[cfg(test)]
mod test {
    use crate::config::models::source::cargo::TrunkMetadata;
    use serde_json::Value;

    #[test]
    fn test_null() {
        let TrunkMetadata { configuration: _ } = serde_json::from_value::<Option<_>>(Value::Null)
            .expect("must not fail")
            .unwrap_or_default();
    }
}
