//! Common functionality and types.

use anyhow::{anyhow, Result};

/// Get the CWD, with more descriptive error handling.
pub async fn get_cwd() -> Result<std::path::PathBuf> {
    std::env::current_dir()
        .map_err(|_| anyhow!("failed to determine current working directory"))
}

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> String {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    format!("{}{}{}", prefix, val, suffix)
}
