use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub metadata: Metadata,
}

impl WorkspaceConfig {
    pub async fn new(manifest: &Path) -> Result<Self> {
        let mut cmd = MetadataCommand::new();
        cmd.manifest_path(dunce::simplified(manifest));
        let metadata = spawn_blocking(move || cmd.exec())
            .await
            .context("error awaiting spawned cargo metadata task")?
            .context("error getting cargo metadata")?;
        Ok(Self { metadata })
    }

    pub fn get_default_workspace(self) -> Option<PathBuf> {
        if let Some(default_member) = self.metadata.workspace_default_members.first() {
            if let Some(found) = self
                .metadata
                .packages
                .into_iter()
                .find(|p| p.id == *default_member)
            {
                return Some(found.manifest_path.clone().into());
            }
        }
        None
    }

    pub fn get_workspace_by_name(self, name: &str) -> Option<PathBuf> {
        // we search for the package in the workspace packages list
        if let Some(one_package) = self.metadata.packages.iter().find(|m| m.name == name) {
            // we check if the package is present in the workspace members list
            if self
                .metadata
                .workspace_members
                .into_iter()
                .any(|p| p == one_package.id)
            {
                // we return the manifest path of the package
                return Some(one_package.manifest_path.clone().into());
            }
        }
        None
    }
}
