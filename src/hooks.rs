use crate::{config::rt::RtcBuild, pipelines::PipelineStage};
use anyhow::{bail, Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::{process::Stdio, sync::Arc};
use tokio::{process::Command, task::JoinHandle};

/// A `FuturesUnordered` containing a `JoinHandle` for each hook-running task.
pub type HookHandles = FuturesUnordered<JoinHandle<Result<()>>>;

/// Spawns tokio tasks for all hooks configured for the given `HookStage`.
pub fn spawn_hooks(cfg: Arc<RtcBuild>, stage: PipelineStage) -> HookHandles {
    let futures: FuturesUnordered<_> = cfg
        .hooks
        .iter()
        .filter(|hook_cfg| hook_cfg.stage == stage)
        .map(|hook_cfg| {
            let mut command = Command::new(hook_cfg.command());

            command
                .current_dir(&cfg.core.working_directory)
                .args(hook_cfg.command_arguments())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .env("TRUNK_PROFILE", if cfg.release { "release" } else { "debug" })
                .env("TRUNK_HTML_FILE", &cfg.target)
                .env("TRUNK_SOURCE_DIR", &cfg.target_parent)
                .env("TRUNK_STAGING_DIR", &cfg.staging_dist)
                .env("TRUNK_DIST_DIR", &cfg.final_dist)
                .env("TRUNK_PUBLIC_URL", &cfg.public_url);

            tracing::info!(command_arguments = ?hook_cfg.command_arguments(), "spawned hook {}", hook_cfg.command());

            let command_name = hook_cfg.command().clone();
            tracing::info!(?stage, command = %command_name, "spawning hook");
            tokio::spawn(async move {
                let status = command
                    .spawn()
                    .with_context(|| format!("error spawning hook call for {}", command_name))?
                    .wait()
                    .await
                    .with_context(|| format!("error calling hook to {}", command_name))?;
                if !status.success() {
                    bail!("hook call to {} returned a bad status", command_name);
                }
                tracing::info!("finished hook {}", command_name);
                Ok(())
            })
        })
        .collect();

    futures
}

/// Waits for all of the given hooks to finish.
pub async fn wait_hooks(mut futures: HookHandles) -> Result<()> {
    while let Some(result) = futures.next().await {
        result??;
    }

    Ok(())
}
