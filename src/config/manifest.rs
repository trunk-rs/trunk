use std::path::Path;

use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use tokio::task::spawn_blocking;

/// A wrapper around the cargo project's metadata.
#[derive(Clone, Debug)]
pub struct CargoMetadata {
    /// The metadata parsed from the cargo project.
    pub metadata: Metadata,
    /// The metadata package info on this package.
    pub package: Package,
    /// The manifest path of the target Cargo.toml.
    pub manifest_path: String,
}

impl CargoMetadata {
    // Create a new instance from the Cargo.toml at the given path.
    pub async fn new(manifest: &Path) -> Result<Self> {
        let mut cmd = MetadataCommand::new();
        cmd.manifest_path(dunce::simplified(manifest));
        let metadata = spawn_blocking(move || cmd.exec())
            .await
            .context("error awaiting spawned cargo metadata task")?
            .context("error getting cargo metadata")?;

        Self::from_metadata(metadata)
    }

    pub(crate) fn from_metadata(metadata: Metadata) -> Result<Self> {
        let package = metadata
            .root_package()
            .cloned()
            .context("could not find the root package of the target crate")?;

        // Get the path to the Cargo.toml manifest.
        let manifest_path = package.manifest_path.to_string();

        Ok(Self {
            metadata,
            package,
            manifest_path,
        })
    }
}
