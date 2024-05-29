use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand};
use std::path::Path;
use std::path::PathBuf;
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        return Ok(Self { metadata });
    }

    pub fn get_default_workspace(self) -> Option<PathBuf> {
        if let Some(default_members) = self.metadata.workspace_default_members.get(0) {
            if let Some(found) = self
                .metadata
                .packages
                .into_iter()
                .find(|p| p.id == *default_members)
            {
                return Some(found.manifest_path.clone().into());
            }
        }
        None
    }
}

/// Load the trunk configuration from the cargo manifest
pub async fn workspace_from_manifest(file: impl AsRef<Path>) -> anyhow::Result<WorkspaceConfig> {
    let workspace_config = WorkspaceConfig::new(file.as_ref()).await?;
    Ok(workspace_config)
}
