//! Common functionality and types.

use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use console::Emoji;
use once_cell::sync::Lazy;
use tokio::fs;
use tokio::process::Command;

pub static BUILDING: Emoji<'_, '_> = Emoji("📦", "");
pub static SUCCESS: Emoji<'_, '_> = Emoji("✅", "");
pub static ERROR: Emoji<'_, '_> = Emoji("❌", "");
pub static SERVER: Emoji<'_, '_> = Emoji("📡", "");

static CWD: Lazy<PathBuf> =
    Lazy::new(|| std::env::current_dir().expect("error getting current dir"));

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> String {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    format!("{}{}{}", prefix, val, suffix)
}

/// A utility function to recursively copy a directory.
pub async fn copy_dir_recursive<F, T>(from_dir: F, to_dir: T) -> Result<()>
where
    F: AsRef<Path> + Debug + Send + 'static,
    T: AsRef<Path> + Send + 'static,
{
    if !path_exists(&from_dir).await? {
        return Err(anyhow!(
            "directory can not be copied as it does not exist {:?}",
            &from_dir
        ));
    }

    tokio::task::spawn_blocking(move || -> Result<()> {
        let opts = fs_extra::dir::CopyOptions {
            overwrite: true,
            content_only: true,
            ..Default::default()
        };
        let _ = fs_extra::dir::copy(from_dir, to_dir, &opts).context("error copying directory")?;
        Ok(())
    })
    .await
    .context("error awaiting spawned copy dir call")?
    .context("error copying directory")
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
    fs::metadata(path.as_ref())
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
                "error checking for existance of path at {:?}",
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
