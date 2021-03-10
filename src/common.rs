//! Common functionality and types.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_std::fs;
use async_std::task::spawn_blocking;

use console::Emoji;

pub static BUILDING: Emoji<'_, '_> = Emoji("📦", "");
pub static SUCCESS: Emoji<'_, '_> = Emoji("✅", "");
pub static ERROR: Emoji<'_, '_> = Emoji("❌", "");
pub static SERVER: Emoji<'_, '_> = Emoji("📡", "");

lazy_static::lazy_static! {
    static ref CWD: PathBuf = std::env::current_dir().expect("error getting current dir");
}

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> String {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    format!("{}{}{}", prefix, val, suffix)
}

/// A utility function to recursively copy a directory.
pub async fn copy_dir_recursive(from_dir: PathBuf, to_dir: PathBuf) -> Result<()> {
    if !path_exists(&from_dir).await? {
        return Err(anyhow!("directory can not be copied as it does not exist {:?}", &from_dir));
    }

    spawn_blocking(move || -> Result<()> {
        let opts = fs_extra::dir::CopyOptions {
            overwrite: true,
            content_only: true,
            ..Default::default()
        };
        let _ = fs_extra::dir::copy(from_dir, to_dir, &opts).context("error copying directory")?;
        Ok(())
    })
    .await
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
    spawn_blocking(move || {
        ::remove_dir_all::remove_dir_all(from_dir.as_path()).context("error removing directory")?;
        Ok(())
    })
    .await
}

/// Checks if path exists.
pub async fn path_exists(path: impl AsRef<Path>) -> Result<bool> {
    let exists = fs::metadata(path.as_ref())
        .await
        .map(|_| true)
        .or_else(|error| if error.kind() == ErrorKind::NotFound { Ok(false) } else { Err(error) })
        .with_context(|| format!("error checking for existance of path at {:?}", path.as_ref()))?;
    Ok(exists)
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
