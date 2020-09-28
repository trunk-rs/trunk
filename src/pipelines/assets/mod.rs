use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, bail, ensure, Result};
use async_std::fs;
use indicatif::ProgressBar;

use crate::common::ERROR;

/// An asset type descriptor extracted from the source HTML.
pub enum AssetType {
    Link {
        /// The `rel` attribute of the HTML link.
        rel: String,
    },
}

/// An asset file to be processed by some build pipeline.
pub struct AssetFile {
    /// The canonicalized path to the target file.
    pub path: PathBuf,
    /// The name of the file itself.
    pub file_name: OsString,
    /// The file stem of the asset file.
    pub file_stem: OsString,
    /// The extension of the file.
    pub ext: String,
    /// The asset's type.
    pub atype: AssetType,
    /// The ID which this asset should use.
    pub id: String,
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
    pub async fn new(path: PathBuf, atype: AssetType, id: String, progress: &ProgressBar) -> Result<Self> {
        // Take the path to referenced resource, if it is actually an FS path, then we continue.
        let path = match fs::canonicalize(&path).await {
            Ok(path) => path,
            Err(_) => {
                if !path.to_string_lossy().contains("://") {
                    progress.println(format!("{}skipping invalid path: {}", ERROR, path.to_string_lossy()));
                }
                return Err(anyhow!("skipping asset which is not a valid path"));
            }
        };
        ensure!(path.is_file().await, "target file does not exist on the FS");
        let file_name = match path.file_name() {
            Some(file_name) => file_name.to_owned(),
            None => bail!("asset has no file name"),
        };
        let file_stem = match path.file_stem() {
            Some(file_stem) => file_stem.to_owned(),
            None => bail!("asset has no file name stem"),
        };
        let ext = match path.extension() {
            Some(ext) => ext.to_string_lossy().to_lowercase(),
            None => bail!("asset has no file extension"),
        };
        Ok(Self {
            path: path.into(),
            file_name,
            file_stem,
            ext,
            atype,
            id,
        })
    }
}

/// The output of an asset pipeline.
pub struct AssetPipelineOutput {
    /// The ID of the asset pipeline.
    pub id: String,
    /// The file name of the output file written to the dist dir (not a full path).
    pub file_name: String,
    /// A bool indicating if the HTML node associated with this pipeline should be removed.
    pub remove: bool,
}
