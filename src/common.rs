//! Common functionality and types.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tokio::task::spawn_blocking;

use console::Emoji;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs;

pub static BUILDING: Emoji<'_, '_> = Emoji("📦", "");
pub static SUCCESS: Emoji<'_, '_> = Emoji("✅", "");
pub static ERROR: Emoji<'_, '_> = Emoji("❌", "");
pub static SERVER: Emoji<'_, '_> = Emoji("📡", "");

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> String {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    format!("{}{}{}", prefix, val, suffix)
}

/// A utility function to recursively copy a directory.
pub async fn copy_dir_recursive(from_dir: PathBuf, to_dir: PathBuf) -> Result<()> {
    // tokio#3373 would provide a better API for checking if a path exists
    if fs::metadata(&from_dir).await.is_err() {
        return Err(anyhow!("directory can not be copied as it does not exist {:?}", &from_dir));
    }

    spawn_blocking(move || {
        let opts = fs_extra::dir::CopyOptions {
            overwrite: true,
            content_only: true,
            ..Default::default()
        };
        let _ = fs_extra::dir::copy(from_dir, to_dir, &opts).context("error copying directory")?;
        Ok(())
    })
    .await
    .context("error copying directory")?
}

/// Build system spinner.
pub fn spinner() -> ProgressBar {
    let style = ProgressStyle::default_spinner().template("{spinner} {prefix} trunk | {wide_msg}");
    ProgressBar::new_spinner().with_style(style)
}
