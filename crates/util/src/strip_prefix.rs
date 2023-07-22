use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

static CWD: Lazy<PathBuf> =
    Lazy::new(|| std::env::current_dir().expect("error getting current dir"));

/// Strip the CWD prefix from the given path.
///
/// Returns `target` unmodified if an error is returned from the operation.
pub fn strip_prefix(target: &Path) -> &Path {
    match target.strip_prefix(CWD.as_path()) {
        Ok(relative) => relative,
        Err(_) => target,
    }
}
