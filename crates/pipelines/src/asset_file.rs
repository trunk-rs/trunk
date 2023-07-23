//! This type does not appear to be shared by things outside of pipelines.
//! So it should be kept private and the copy in main crate should be removed once all pipelines are
//! migrated.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tokio::fs;
use trunk_util::ResultExt;

use crate::util::{ErrorReason, Result};

/// An asset file to be processed by some build pipeline.
pub(crate) struct AssetFile {
    /// The canonicalized path to the target file.
    pub path: PathBuf,
    /// The name of the file itself.
    pub file_name: OsString,
    /// The file stem of the asset file.
    pub file_stem: OsString,
    /// The extension of the file.
    pub ext: Option<String>,
}

impl AssetFile {
    /// Create a new instance.
    ///
    /// The given path will be validated to ensure the following:
    /// - that the full canonicalized path points to a file on the FS.
    /// - that the file has a filename.
    /// - that the file has an extension.
    ///
    /// Any errors returned from this constructor indicate that one of these invariants was not
    /// upheld.
    pub async fn new(rel_dir: &Path, mut path: PathBuf) -> Result<Self> {
        // If the given path is not absolute, then we join it with the directory from which the
        // relative path should be based.
        if !path.is_absolute() {
            path = rel_dir.join(path);
        }

        // Take the path to referenced resource, if it is actually an FS path, then we continue.
        let path = fs::canonicalize(&path)
            .await
            // canonicalize only fails if that file does not exist or a non terminating component is
            // not a directory. In both cases, we can tell user that the file does not
            // exist.
            .with_reason(|| ErrorReason::FsNotExist { path: path.clone() })?;

        let file_name = match path.file_name() {
            Some(file_name) => file_name.to_owned(),
            None => return Err(ErrorReason::PathNoFileName { path }.into_error()),
        };
        let file_stem = match path.file_stem() {
            Some(file_stem) => file_stem.to_owned(),
            None => return Err(ErrorReason::PathNoFileStem { path }.into_error()),
        };
        let ext = path
            .extension()
            .map(|ext| ext.to_owned().to_string_lossy().to_string());
        Ok(Self {
            path,
            file_name,
            file_stem,
            ext,
        })
    }

    /// Copy this asset to the target dir. If hashing is enabled, create a hash from the file
    /// contents and include it as hex string in the destination file name.
    ///
    /// The base file name (stripped path, without any parent folders) is returned if the operation
    /// was successful.
    pub async fn copy(&self, to_dir: &Path, with_hash: bool) -> Result<String> {
        let bytes = fs::read(&self.path)
            .await
            .with_reason(|| ErrorReason::FsReadFailed {
                path: self.path.clone(),
            })?;

        let file_name = if with_hash {
            format!(
                "{}-{:x}.{}",
                &self.file_stem.to_string_lossy(),
                seahash::hash(bytes.as_ref()),
                &self.ext.as_deref().unwrap_or_default()
            )
        } else {
            self.file_name.to_string_lossy().into_owned()
        };

        let file_path = to_dir.join(&file_name);

        fs::copy(&self.path, &file_path)
            .await
            .with_reason(move || ErrorReason::FsCopyFailed {
                from_path: self.path.clone(),
                to_path: file_path,
            })?;

        Ok(file_name)
    }

    /// Read the content of this asset to a String.
    pub async fn read_to_string(&self) -> Result<String> {
        fs::read_to_string(&self.path)
            .await
            .with_reason(|| ErrorReason::FsReadFailed {
                path: self.path.clone(),
            })
    }
}
