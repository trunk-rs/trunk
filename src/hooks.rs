use crate::{config::rt::RtcBuild, pipelines::PipelineStage};
use anyhow::{bail, Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
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
            let current_dir = if cfg!(target_os = "windows") {
                try_strip_windows_unc_prefix(&cfg.core.working_directory)
            } else {
                cfg.core.working_directory.clone()
            };

            command
                .current_dir(current_dir)
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

/// Tries to strip the UNC prefix (`\\\\?\\`) from Windows paths.
/// If the path is not prefixed by a UNC, or the UNC can not be stripped safely,
/// the original path is returned.
///
/// # See also
/// + [Issue #889](https://github.com/trunk-rs/trunk/issues/889) for details.
/// + The [`dunce` crate](https://crates.io/crates/dunce) which was used as reference.
///     **Note:** `dunce` is under teh CC0 license, must check for compaitiblity if included.
fn try_strip_windows_unc_prefix(path: impl AsRef<Path>) -> PathBuf {
    if is_safe_to_strip_unc_prefix(&path) {
        path.as_ref()
            .to_str()
            .and_then(|s| s.get(4..))
            .map(PathBuf::from)
            .unwrap_or(path.as_ref().to_path_buf())
    } else {
        path.as_ref().to_path_buf()
    }
}

fn is_safe_to_strip_unc_prefix(path: impl AsRef<Path>) -> bool {
    use std::path::{Component, Prefix};

    let mut components = path.as_ref().components();
    match components.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::VerbatimDisk(..) => {}
            _ => return false,
        },
        _ => return false,
    }

    if path.as_ref().as_os_str().len() > 260 {
        return false;
    }

    true
}
