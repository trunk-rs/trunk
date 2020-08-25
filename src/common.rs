//! Common functionality and types.

use anyhow::{anyhow, Result};

/// Get the CWD, with more descriptive error handling.
pub async fn get_cwd() -> Result<std::path::PathBuf> {
    std::env::current_dir()
        .map_err(|_| anyhow!("failed to determine current working directory"))
}
