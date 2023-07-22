//! Common functionality and types.

use std::convert::Infallible;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use console::Emoji;
use once_cell::sync::Lazy;
use tokio::fs;

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
                "error checking for existence of path at {:?}",
                path.as_ref()
            )
        })
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
