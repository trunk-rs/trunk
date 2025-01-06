//! Common functionality and types.
pub mod html_rewrite;

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose, Engine};
use console::Emoji;
use once_cell::sync::Lazy;
use rand::RngCore;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;

pub static BUILDING: Emoji = Emoji("📦 ", "");
pub static SUCCESS: Emoji = Emoji("✅ ", "");
pub static ERROR: Emoji = Emoji("❌ ", "");
pub static SERVER: Emoji = Emoji("📡 ", "");
pub static LOCAL: Emoji = Emoji("🏠 ", "");
pub static NETWORK: Emoji = Emoji("💻 ", "");
pub static STARTING: Emoji = Emoji("🚀 ", "");
#[cfg(feature = "update_check")]
pub static UPDATE: Emoji = Emoji("⏫ ", "");

// If we fail to get the current_dir, we can't do much and just fail, so we can use expect(..).
#[allow(clippy::expect_used)]
static CWD: Lazy<PathBuf> =
    Lazy::new(|| std::env::current_dir().expect("error getting current dir"));

/// A utility function to recursively copy a directory.
pub async fn copy_dir_recursive<F, T>(from_dir: F, to_dir: T) -> Result<HashSet<PathBuf>>
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

    let mut collector = HashSet::new();

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
            let files = Box::pin(async move {
                copy_dir_recursive(entry.path(), to.join(entry.file_name())).await
            })
            .await?;
            collector.extend(files);
        } else {
            let to = to.join(entry.file_name());
            // Does overwrite!
            tokio::fs::copy(entry.path(), &to).await?;
            collector.insert(to);
        }
    }

    Ok(collector)
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
        ::remove_dir_all::remove_dir_all(from_dir).context("error removing directory")?;
        Ok(())
    })
    .await
    .context("error awaiting spawned remove dir call")?
}

/// Checks if path exists.
pub async fn path_exists(path: impl AsRef<Path>) -> Result<bool> {
    path_exists_and(path, |_| true).await
}

/// Checks if path exists and metadata matches the given predicate.
pub async fn path_exists_and(
    path: impl AsRef<Path>,
    and: impl FnOnce(Metadata) -> bool,
) -> Result<bool> {
    tokio::fs::metadata(path.as_ref())
        .await
        .map(and)
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
    let has_executable_flag = |_meta: Metadata| true;

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
    target.strip_prefix(CWD.as_path()).unwrap_or(target)
}

/// Run a global command with the given arguments and make sure it completes successfully. If it
/// fails an error is returned.
#[tracing::instrument(level = "trace", skip(name, args))]
pub async fn run_command(
    name: &str,
    path: impl AsRef<Path> + Debug,
    args: &[impl AsRef<OsStr> + Debug],
    working_dir: impl AsRef<Path> + Debug,
) -> Result<()> {
    tracing::debug!(?args, "{name} args");

    let path = path.as_ref();

    let status = Command::new(path)
        .current_dir(working_dir.as_ref())
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| {
            format!(
                "error running {name} using executable '{}' with args: '{args:?}'",
                path.display(),
            )
        })?
        .wait()
        .await
        .with_context(|| format!("error during {name} call"))?;

    if !status.success() {
        bail!(
            "{name} call to executable '{}' with args: '{args:?}' returned a bad status: {status}",
            path.display()
        );
    }

    Ok(())
}

/// Handle invocation errors indicating that the target binary was not found, simply wrapping the
/// error in additional context stating more clearly that the target was not found.
pub fn check_target_not_found_err(err: anyhow::Error, target: &str) -> anyhow::Error {
    let io_err: &std::io::Error = match err.downcast_ref() {
        Some(io_err) => io_err,
        None => return err,
    };
    match io_err.kind() {
        std::io::ErrorKind::NotFound => err.context(format!("'{}' not found", target)),
        _ => err,
    }
}

/// Create a target path from a base and an optional relative prefix.
///
/// This is intended for cases where a subdirectory for a target base (like `dist`) is being
/// composed. The target directory will also be created.
pub async fn target_path(
    base: &Path,
    target_path: Option<&Path>,
    default: Option<&OsStr>,
) -> Result<PathBuf> {
    if let Some(path) = target_path {
        if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            bail!(
                "Invalid data-target-path '{}'. Must be a relative path without '..'.",
                path.display()
            );
        }
        let dir_out = base.join(path);
        tokio::fs::create_dir_all(&dir_out).await?;
        Ok(dir_out)
    } else if let Some(default) = default {
        Ok(base.join(default))
    } else {
        Ok(base.to_owned())
    }
}

/// Create a file_name, including the relative base to the `dist`.
///
/// The function will return an error if the `target_file` is not a direct or indirect child of
/// `dist`.
pub fn dist_relative(dist: &Path, target_file: &Path) -> Result<String> {
    let target_file = target_file.strip_prefix(dist).with_context(|| {
        format!(
            "unable to create a relative path of '{}' in '{}'",
            target_file.display(),
            dist.display()
        )
    })?;

    Ok(path_to_href(target_file))
}

/// Take a path, and create a relocated name it into the `target_path`, if present.
pub fn apply_data_target_path(path: impl Into<String>, target_path: &Option<PathBuf>) -> String {
    match target_path {
        Some(target_path) => path_to_href(target_path.join(path.into())),
        None => path.into(),
    }
}

/// Take a path and turn it into an href compatible path
///
/// Basically, this means replacing path separator with a forward slash on Windows.
pub fn path_to_href(path: impl AsRef<Path>) -> String {
    let path = path
        .as_ref()
        .iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>();
    path.join("/")
}

/// A nonce random generator for script and style
///
/// https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/nonce
pub fn nonce() -> String {
    let mut buffer = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut buffer);
    general_purpose::STANDARD.encode(buffer)
}

/// Creates the 'nonce' attribute.
///
/// Result is intented to be placed immediately without any spacing after the
/// html tag or other attributes.
pub fn nonce_attr(attr: &Option<String>) -> String {
    match attr {
        Some(v) => format!(r#" nonce="{v}""#),
        None => "".to_string(),
    }
}
