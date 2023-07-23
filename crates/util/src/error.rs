use std::path::PathBuf;
use std::process::ExitStatus;

use derive_more::Display;
use thiserror::Error;

/// Reasons why Error happened.
#[derive(Debug, Display)]
pub enum ErrorReason {
    /// failed to copy file to target
    #[display(
        fmt = "failed to copy from {} to {}",
        "from_path.display()",
        "to_path.display()"
    )]
    FsCopyFailed {
        from_path: PathBuf,
        to_path: PathBuf,
    },
    /// failed to read file
    #[display(fmt = "failed to read {}", "path.display()")]
    FsReadFailed { path: PathBuf },
    /// failed to delete file
    #[display(fmt = "failed to remove {}", "path.display()")]
    FsRemoveFailed { path: PathBuf },
    /// failed to write file
    #[display(fmt = "failed to write {}", "path.display()")]
    FsWriteFailed { path: PathBuf },
    /// file not exist
    #[display(fmt = "file {} does not exist", "path.display()")]
    FsNotExist { path: PathBuf },

    /// path does not have a file name
    #[display(fmt = "path {} does not have a file name", "path.display()")]
    PathNoFileName { path: PathBuf },
    /// path does not have a file stem
    #[display(fmt = "path {} does not have a file stem", "path.display()")]
    PathNoFileStem { path: PathBuf },

    /// error decompressing archive
    #[display(fmt = "error decompressing archive")]
    ArchiveOther,
    /// error checking file mode
    #[display(fmt = "error checking file mode for file {}", "_0.display()")]
    FileMode(PathBuf),
    /// file not found in archive
    #[display(fmt = "file not found in archive")]
    ArchiveFileNotFound,
    /// failed to copy from archive
    #[display(fmt = "failed to copy from archive")]
    ArchiveCopyFailed,
    /// failed to seek archive
    #[display(fmt = "failed to seek archive")]
    ArchiveSeekFailed,
    /// failed to get archive entries
    #[display(fmt = "failed to get archive entries")]
    ArchiveGetEntryFailed,
    /// failed to extracting files
    #[display(fmt = "failed to extract files")]
    ArchiveExtractFailed,
    /// failed to set permission
    #[display(fmt = "failed to set permission")]
    ArchiveSetPermissionFailed,

    /// failed to parse version
    #[display(
        fmt = "failed to parse version, missing or malformed version output: {}",
        "_0"
    )]
    ToolchainMalformedVersion(String),
    /// failed downloading release archive
    #[display(fmt = "failed downloading release archive")]
    ToolchainDownloadFailed,
    /// failed writing file downloaded
    #[display(fmt = "failed writing file downloaded")]
    ToolchainWriteFailed,
    /// failed opening downloaded file
    #[display(fmt = "failed opening downloaded file")]
    ToolchainOpenFailed,
    /// failed deleting temporary archive
    #[display(fmt = "failed deleting temporary archive")]
    ToolchainDeleteFailed,
    /// failed to run command
    #[display(fmt = "running command `{}` failed", "_0")]
    ToolchainCommandFailed(String),
    /// failed to find command
    #[display(fmt = "failed to find command executable")]
    ToolchainFileNotFound,
    /// failed creating cache directory
    #[display(fmt = "failed creating cache directory")]
    ToolchainCreateCacheFailed,

    /// Current target is not supported for auto toolchain downloading
    #[display(fmt = "current target is not supported")]
    ToolchainUnsupportedTarget,

    /// Tokio task failed to join
    #[display(fmt = "tokio task has failed to join")]
    TokioTaskFailed,

    /// command has failed
    #[display(fmt = "Command {} has failed to run, status {:?}", "name", "status")]
    ExecutableRunFailed {
        name: String,
        status: Option<ExitStatus>,
    },

    /// command not found
    #[display(fmt = "failed to find command {} ", "name")]
    ExecutableNotFound { name: String },

    /// failed to find src attribute for `<script data-trunk ... />`.
    #[display(fmt = r#"required attr `src` missing for <script data-trunk .../> element"#)]
    PipelineScriptSrcNotFound,

    /// failed to find href attribute for `<link data-trunk ... />`.
    #[display(
        fmt = r#"required attr `href` missing for <link data-trunk rel="css|sass|tailwind-css" .../> element"#
    )]
    PipelineLinkHrefNotFound,
}

impl ErrorReason {
    /// Turns a reason into an error with no source error.
    pub fn into_error(self) -> Error {
        Error {
            source: None,
            reason: self,
        }
    }
}

/// Error emitted by trunk-util
#[derive(Error, Debug)]
#[error("{reason}", reason = .reason)]
pub struct Error {
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    reason: ErrorReason,
}

/// Error extensions to make it easier to work with existing errors.
pub trait ErrorExt {
    /// Add a reason to an existing error, making it a type of [`Error`].
    fn reason(self, reason: ErrorReason) -> Error;

    /// Similar to reason(), but the reason is created with a closure dynamically.
    fn with_reason<R>(self, with_reason: R) -> Error
    where
        R: FnOnce() -> ErrorReason;
}

/// Result extensions to make it easier to work with existing results.
pub trait ResultExt<T> {
    /// Add a reason to an existing error, making it a type of [`Error`].
    fn reason(self, reason: ErrorReason) -> Result<T>;

    /// Similar to reason(), but the reason is created with a closure dynamically.
    fn with_reason<R>(self, with_reason: R) -> Result<T>
    where
        R: FnOnce() -> ErrorReason;
}

impl<E> ErrorExt for E
where
    E: 'static + std::error::Error + Send + Sync,
{
    fn reason(self, reason: ErrorReason) -> Error {
        Error {
            source: Some(Box::new(self)),
            reason,
        }
    }

    fn with_reason<R>(self, with_reason: R) -> Error
    where
        R: FnOnce() -> ErrorReason,
    {
        self.reason(with_reason())
    }
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: 'static + std::error::Error + Send + Sync,
{
    fn reason(self, reason: ErrorReason) -> Result<T> {
        self.map_err(|e| Error {
            source: Some(Box::new(e)),
            reason,
        })
    }

    fn with_reason<R>(self, with_reason: R) -> Result<T>
    where
        R: FnOnce() -> ErrorReason,
    {
        self.map_err(move |e| Error {
            source: Some(Box::new(e)),
            reason: with_reason(),
        })
    }
}

impl<T> ResultExt<T> for std::option::Option<T> {
    fn reason(self, reason: ErrorReason) -> Result<T> {
        self.ok_or_else(|| Error {
            source: None,
            reason,
        })
    }

    fn with_reason<R>(self, with_reason: R) -> Result<T>
    where
        R: FnOnce() -> ErrorReason,
    {
        self.ok_or_else(move || Error {
            source: None,
            reason: with_reason(),
        })
    }
}

pub type Result<T> = std::result::Result<T, Error>;
