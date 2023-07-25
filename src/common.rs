//! Common functionality and types.

use std::convert::Infallible;

use anyhow::Result;
use console::Emoji;

pub static BUILDING: Emoji<'_, '_> = Emoji("📦", "");
pub static SUCCESS: Emoji<'_, '_> = Emoji("✅", "");
pub static ERROR: Emoji<'_, '_> = Emoji("❌", "");
pub static SERVER: Emoji<'_, '_> = Emoji("📡", "");
pub static LOCAL: Emoji<'_, '_> = Emoji("🏠", "");
pub static NETWORK: Emoji<'_, '_> = Emoji("💻", "");

/// Ensure the given value for `--public-url` is formatted correctly.
pub fn parse_public_url(val: &str) -> Result<String, Infallible> {
    let prefix = if !val.starts_with('/') { "/" } else { "" };
    let suffix = if !val.ends_with('/') { "/" } else { "" };
    Ok(format!("{}{}{}", prefix, val, suffix))
}
