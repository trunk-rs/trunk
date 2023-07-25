use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use tokio::fs;

use crate::{ErrorReason, Result, ResultExt};

/// Checks if path exists.
pub async fn path_exists(path: impl AsRef<Path>) -> Result<bool> {
    let path = path.as_ref();
    fs::metadata(path)
        .await
        .map(|_| true)
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_reason(|| ErrorReason::FsReadFailed {
            path: path.to_owned(),
        })
}

/// A utility function to recursively copy a directory.
pub async fn copy_dir_recursive<F, T>(from_dir: F, to_dir: T) -> Result<()>
where
    F: Into<PathBuf>,
    T: Into<PathBuf>,
{
    let from_dir = from_dir.into();
    let to_dir = to_dir.into();

    if !path_exists(&from_dir).await? {
        return Err(ErrorReason::FsNotExist {
            path: from_dir.to_owned(),
        }
        .into_error());
    }
    {
        let from_dir = from_dir.clone();
        let to_dir = to_dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let opts = fs_extra::dir::CopyOptions {
                overwrite: true,
                content_only: true,
                ..Default::default()
            };
            let _ = fs_extra::dir::copy(&from_dir, &to_dir, &opts).with_reason(|| {
                ErrorReason::FsCopyFailed {
                    from_path: from_dir,
                    to_path: to_dir,
                }
            })?;
            Ok(())
        })
    }
    .await
    .reason(ErrorReason::TokioTaskFailed)?
    .with_reason(|| ErrorReason::FsCopyFailed {
        from_path: from_dir.to_owned(),
        to_path: to_dir.to_owned(),
    })
}
