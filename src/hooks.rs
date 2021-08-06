use std::{process::Stdio, sync::Arc};

use anyhow::{bail, Context, Result};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use serde::Deserialize;
use tokio::process::Command;

use crate::config::RtcBuild;

pub async fn spawn_hooks(cfg: Arc<RtcBuild>, stage: HookStage) -> Result<()> {
    let mut futures: FuturesUnordered<_> = cfg
        .hooks
        .iter()
        .filter(|hook_cfg| hook_cfg.stage == stage)
        .map(|hook_cfg| -> Result<_> {
            let mut child = Command::new(&hook_cfg.command)
                .args(&hook_cfg.command_arguments)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .env("TRUNK_STAGING_DIR", &cfg.staging_dist)
                .spawn()
                .with_context(|| format!("error spawning hook call for {}", hook_cfg.command))?;

            tracing::info!(command_arguments = ?hook_cfg.command_arguments, "spawned hook {}", hook_cfg.command);

            let command_name = hook_cfg.command.clone();

            let join_handle = tokio::spawn(async move {
                let status = child
                    .wait()
                    .await
                    .with_context(|| format!("error calling hook to {}", command_name))?;
                if !status.success() {
                    bail!("hook call to {} returned a bad status", command_name);
                }
                tracing::info!("finished hook {}", command_name);
                Ok(())
            });

            Ok(join_handle)
        })
        .collect::<Result<_>>()?;

    while let Some(result) = futures.next().await {
        result??;
    }

    Ok(())
}

/// A stage stage in the build process.
///
/// This is used to specify when a hook will run.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookStage {
    Asset,
}
