use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;

use futures_util::future::ready;
use futures_util::TryFutureExt;
use tokio::fs;
use tokio::process::Command;

use crate::error::ResultExt;
use crate::{ErrorExt, ErrorReason, Result};

/// Represents an Executable
#[derive(Debug, Clone)]
pub struct Executable {
    /// The name of the executable
    name: Option<Cow<'static, str>>,
    /// The path of the executable
    path: Cow<'static, Path>,
}

impl Executable {
    /// Creates an Executable
    pub fn new<P>(path: P) -> Self
    where
        P: Into<Cow<'static, Path>>,
    {
        Executable {
            name: None,
            path: path.into(),
        }
    }

    /// Gives current executable an alternative name, used for debugging
    pub fn with_name<S>(self, name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Executable {
            name: Some(name.into()),
            path: self.path,
        }
    }

    /// Returns name of current executable
    pub fn name(&self) -> Option<Cow<'_, str>> {
        if let Some(m) = self.name.as_deref() {
            return Some(Cow::Borrowed(m));
        }

        if let Some(m) = self.path.file_name() {
            return Some(m.to_string_lossy());
        }

        None
    }

    /// Returns a tokio command struct of current executable.
    pub fn command(&self) -> Command {
        Command::new(self.path.as_ref())
    }

    /// Run a global command with the given arguments and make sure it completes successfully. If it
    /// fails an error is returned.
    #[tracing::instrument(level = "trace", skip(self, args))]
    pub async fn run_with_args(&self, args: &[impl AsRef<OsStr> + Debug]) -> Result<()> {
        let name = self.name().unwrap_or(Cow::Borrowed("unknown"));
        tracing::debug!(?args, "{name} args");
        let status = ready(
            self.command()
                .args(args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn(),
        )
        .and_then(|mut f| async move { f.wait().await })
        .await
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                // Handle invocation errors indicating that the target binary was not found, simply
                // wrapping the error in additional context stating more clearly
                // that the target was not found.
                e.reason(ErrorReason::ExecutableNotFound {
                    name: name.clone().into_owned(),
                })
            } else {
                e.reason(ErrorReason::ExecutableRunFailed {
                    name: name.clone().into_owned(),
                    status: None,
                })
            }
        })?;
        if !status.success() {
            return Err(ErrorReason::ExecutableRunFailed {
                name: name.into_owned(),
                status: Some(status),
            }
            .into_error());
        }
        Ok(())
    }
}

/// Check whether a given path exists, is a file and marked as executable.
pub async fn is_executable(path: impl AsRef<Path>) -> Result<bool> {
    let path = path.as_ref();
    #[cfg(unix)]
    let has_executable_flag = |meta: Metadata| {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o100 != 0
    };
    #[cfg(not(unix))]
    let has_executable_flag = |meta: Metadata| true;

    fs::metadata(path)
        .await
        .map(|meta| meta.is_file() && has_executable_flag(meta))
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_reason(|| ErrorReason::FileMode(path.to_owned()))
}
