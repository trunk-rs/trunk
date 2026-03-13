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
        let manifest_path = dunce::simplified(manifest).to_path_buf();
        let mut cmd = MetadataCommand::new();
        cmd.manifest_path(&manifest_path);
        let metadata = spawn_blocking(move || cmd.exec())
            .await
            .context("error awaiting spawned cargo metadata task")?
            .context("error getting cargo metadata")?;

        Self::from_metadata_with_manifest_path(metadata, manifest_path)
    }



    /// Create a new instance from metadata with a known manifest path.
    /// This is the preferred method as it can better handle workspace scenarios.
    pub(crate) fn from_metadata_with_manifest_path(metadata: Metadata, original_manifest_path: std::path::PathBuf) -> Result<Self> {
        let package = Self::find_target_package(&metadata, Some(&original_manifest_path))?;

        // Get the path to the Cargo.toml manifest.
        let manifest_path = package.manifest_path.to_string();

        Ok(Self {
            metadata,
            package,
            manifest_path,
        })
    }

    /// Find the target package from metadata, handling both standalone packages and workspace members.
    fn find_target_package(metadata: &Metadata, original_manifest_path: Option<&std::path::Path>) -> Result<Package> {
        // First, try the traditional approach for standalone packages
        if let Some(package) = metadata.root_package() {
            return Ok(package.clone());
        }

        // If no root package exists, we're likely in a workspace.
        // In this case, we need to find the package that corresponds to the manifest path
        // that was used to generate this metadata.

        let workspace_packages = metadata.workspace_packages();

        if workspace_packages.is_empty() {
            anyhow::bail!(
                "could not find the root package of the target crate: no root package found and no workspace members available"
            );
        }

        // If we have the original manifest path, try to find the exact matching package
        if let Some(original_path) = original_manifest_path {
            // Canonicalize the original path for comparison
            let canonical_original = dunce::canonicalize(original_path)
                .with_context(|| format!("failed to canonicalize manifest path: {}", original_path.display()))?;

            for package in &workspace_packages {
                // Canonicalize the package's manifest path for comparison
                if let Ok(canonical_package) = dunce::canonicalize(&package.manifest_path) {
                    if canonical_original == canonical_package {
                        return Ok((*package).clone());
                    }
                }
            }

            // If we couldn't find an exact match, provide helpful error information
            let package_names: Vec<String> = workspace_packages.iter()
                .map(|p| format!("{} ({})", p.name, p.manifest_path))
                .collect();
            anyhow::bail!(
                "could not find the root package of the target crate: manifest path '{}' does not match any workspace member. \
                Available workspace members: [{}]",
                original_path.display(),
                package_names.join(", ")
            );
        }

        // If there's only one workspace package, use it
        if workspace_packages.len() == 1 {
            return Ok(workspace_packages[0].clone());
        }

        // If there are multiple workspace packages and we don't have a specific manifest path,
        // we can't determine which one to use.
        let package_names: Vec<&str> = workspace_packages.iter().map(|p| p.name.as_str()).collect();
        anyhow::bail!(
            "could not find the root package of the target crate: multiple workspace members found: [{}]. \
            Consider running trunk from within a specific package directory or using a more specific manifest path.",
            package_names.join(", ")
        );
    }
}
