use std::path::PathBuf;

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
    /// Get the project's cargo metadata of the CWD, or of the project specified by the given manifest path.
    pub async fn new(manifest: &Option<PathBuf>) -> Result<Self> {
        // Fetch the cargo project's metadata.
        let mut cmd = MetadataCommand::new();
        if let Some(manifest) = manifest.as_ref() {
            cmd.manifest_path(manifest);
        }
        let metadata = spawn_blocking(move || cmd.exec()).await?;

        // Get a handle to this project's package info.
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
