use std::path::Path;

use anyhow::{anyhow, Result};
use async_std::task::spawn_blocking;
use cargo_metadata::{Metadata, MetadataCommand, Package};

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

        let mut metadata = spawn_blocking(move || cmd.exec()).await?;
        metadata.target_directory = metadata.target_directory.canonicalize()?;

        let package = metadata
            .root_package()
            .cloned()
            .ok_or_else(|| anyhow!("could not find root package of the target crate"))?;

        // Get the path to the Cargo.toml manifest.
        let manifest_path = package.manifest_path.to_string_lossy().to_string();

        Ok(Self {
            metadata,
            package,
            manifest_path,
        })
    }
}
