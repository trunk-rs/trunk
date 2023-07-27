use std::path::Path;

use cargo_metadata::{Metadata, MetadataCommand, Package};
use tokio::task::spawn_blocking;

use crate::{ErrorReason, Result, ResultExt};

/// Config options for the cargo build command
#[derive(Clone, Debug)]
pub enum Features {
    /// Use cargo's `--all-features` flag during compilation.
    All,
    /// An explicit list of features to use; might be empty; might include no-default-features.
    Custom {
        /// Space or comma separated list of cargo features to activate.
        features: Option<String>,
        /// Use cargo's `--no-default-features` flag during compilation.
        no_default_features: bool,
    },
}

impl Default for Features {
    fn default() -> Self {
        Features::Custom {
            features: None,
            no_default_features: false,
        }
    }
}

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
            .reason(ErrorReason::CargoMetadataReadFailed)?
            .reason(ErrorReason::CargoMetadataReadFailed)?;

        let package = metadata
            .root_package()
            .cloned()
            .reason(ErrorReason::MetadataNoRootPackageFound)?;

        // Get the path to the Cargo.toml manifest.
        let manifest_path = package.manifest_path.to_string();

        Ok(Self {
            metadata,
            package,
            manifest_path,
        })
    }
}
