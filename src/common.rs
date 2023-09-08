//! Common functionality and types.
use std::convert::Infallible;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use async_recursion::async_recursion;
use console::Emoji;
use once_cell::sync::Lazy;
use tokio::fs;
use tokio::process::Command;

pub static BUILDING: Emoji<'_, '_> = Emoji("📦", "");
pub static SUCCESS: Emoji<'_, '_> = Emoji("✅", "");
pub static ERROR: Emoji<'_, '_> = Emoji("❌", "");
pub static SERVER: Emoji<'_, '_> = Emoji("📡", "");
pub static LOCAL: Emoji<'_, '_> = Emoji("🏠", "");
pub static NETWORK: Emoji<'_, '_> = Emoji("💻", "");

static CWD: Lazy<PathBuf> =
    Lazy::new(|| std::env::current_dir().expect("error getting current dir"));

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> Result<String, Infallible> {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    Ok(format!("{}{}{}", prefix, val, suffix))
}

/// A utility function to recursively copy a directory.
#[async_recursion]
pub async fn copy_dir_recursive<F, T>(from_dir: F, to_dir: T) -> Result<()>
where
    F: AsRef<Path> + Debug + Send + 'static,
    T: AsRef<Path> + Send + 'static,
{
    let from = from_dir.as_ref();
    let to: &Path = to_dir.as_ref();

    // Source must exist and be a directory.
    let from_metadata = tokio::fs::metadata(from).await.with_context(|| {
        format!("Unable to retrieve metadata of '{from:?}'. Path does probably not exist.")
    })?;
    if !from_metadata.is_dir() {
        return Err(anyhow!(
            "Path '{from:?}' can not be copied as it is not a directory!"
        ));
    }

    // Target is created if missing.
    if tokio::fs::metadata(to).await.is_err() {
        tokio::fs::create_dir_all(to)
            .await
            .with_context(|| format!("Unable to create target directory '{to:?}'."))?;
    }

    // Copy files and recursively handle nested directories.
    let mut read_dir = tokio::fs::read_dir(from)
        .await
        .context(anyhow!("Unable to read dir"))?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .context(anyhow!("Unable to read next dir entry"))?
    {
        if entry.file_type().await?.is_dir() {
            copy_dir_recursive(entry.path(), to.join(entry.file_name())).await?;
        } else {
            // Does overwrite!
            tokio::fs::copy(entry.path(), to.join(entry.file_name())).await?;
        }
    }

    Ok(())
}

/// A utility function to recursively delete a directory.
///
/// Use this instead of fs::remove_dir_all(...) because of Windows compatibility issues, per
/// advice of https://blog.qwaz.io/chat/issues-of-rusts-remove-dir-all-implementation-on-windows
pub async fn remove_dir_all(from_dir: PathBuf) -> Result<()> {
    if !path_exists(&from_dir).await? {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || {
        ::remove_dir_all::remove_dir_all(from_dir.as_path()).context("error removing directory")?;
        Ok(())
    })
    .await
    .context("error awaiting spawned remove dir call")?
}

/// Checks if path exists.
pub async fn path_exists(path: impl AsRef<Path>) -> Result<bool> {
    tokio::fs::metadata(path.as_ref())
        .await
        .map(|_| true)
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_context(|| {
            format!(
                "error checking for existence of path at {:?}",
                path.as_ref()
            )
        })
}

/// Check whether a given path exists, is a file and marked as executable.
pub async fn is_executable(path: impl AsRef<Path>) -> Result<bool> {
    #[cfg(unix)]
    let has_executable_flag = |meta: Metadata| {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o100 != 0
    };
    #[cfg(not(unix))]
    let has_executable_flag = |meta: Metadata| true;

    fs::metadata(path.as_ref())
        .await
        .map(|meta| meta.is_file() && has_executable_flag(meta))
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_context(|| format!("error checking file mode for file {:?}", path.as_ref()))
}

/// Strip the CWD prefix from the given path.
///
/// Returns `target` unmodified if an error is returned from the operation.
pub fn strip_prefix(target: &Path) -> &Path {
    match target.strip_prefix(CWD.as_path()) {
        Ok(relative) => relative,
        Err(_) => target,
    }
}

/// Run a global command with the given arguments and make sure it completes successfully. If it
/// fails an error is returned.
#[tracing::instrument(level = "trace", skip(name, path, args))]
pub async fn run_command(
    name: &str,
    path: &Path,
    args: &[impl AsRef<OsStr> + Debug],
) -> Result<()> {
    tracing::debug!(?args, "{name} args");
    let status = Command::new(path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("error spawning {} call", name))?
        .wait()
        .await
        .with_context(|| format!("error during {} call", name))?;
    if !status.success() {
        bail!("{} call returned a bad status", name);
    }
    Ok(())
}
