//! Download management for external tools and applications. Locate and automatically download
//! applications (if needed) to use them in the build pipeline.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, ensure, Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use directories_next::ProjectDirs;
use futures::prelude::*;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncSeekExt, AsyncWriteExt, BufReader, SeekFrom};
use tokio::process::Command;
use tokio_tar::{Archive, Entry};

use crate::common::is_executable;

/// The application to locate and eventually download when calling [`get`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Application {
    /// wasm-bindgen for generating the JS bindings.
    WasmBindgen,
    /// wasm-opt to improve performance and size of the output file further.
    WasmOpt,
}

impl Application {
    /// Base name of the executable without extension.
    fn name(&self) -> &str {
        match self {
            Self::WasmBindgen => "wasm-bindgen",
            Self::WasmOpt => "wasm-opt",
        }
    }

    /// Path of the executable within the downloaded archive.
    fn path(&self) -> &str {
        if cfg!(windows) {
            match self {
                Self::WasmBindgen => "wasm-bindgen.exe",
                Self::WasmOpt => "bin/wasm-opt.exe",
            }
        } else {
            match self {
                Self::WasmBindgen => "wasm-bindgen",
                Self::WasmOpt => "bin/wasm-opt",
            }
        }
    }

    /// Additonal files included in the archive that are required to run the main binary.
    fn extra_paths(&self) -> &[&str] {
        if cfg!(target_os = "macos") && *self == Self::WasmOpt {
            &["lib/libbinaryen.dylib"]
        } else {
            &[]
        }
    }

    /// Default version to use if not set by the user.
    fn default_version(&self) -> &str {
        match self {
            Self::WasmBindgen => "0.2.74",
            Self::WasmOpt => "version_101",
        }
    }

    /// Target for the current OS as part of the download URL. Can fail as there might be no release
    /// for the current platform.
    fn target(&self) -> Result<&str> {
        Ok(match self {
            Self::WasmBindgen => {
                if cfg!(target_os = "windows") {
                    "pc-windows-msvc"
                } else if cfg!(target_os = "macos") {
                    "apple-darwin"
                } else if cfg!(target_os = "linux") {
                    "unknown-linux-musl"
                } else {
                    bail!("unsupported OS")
                }
            }
            Self::WasmOpt => {
                if cfg!(target_os = "windows") {
                    "windows"
                } else if cfg!(target_os = "macos") {
                    "macos"
                } else if cfg!(target_os = "linux") {
                    "linux"
                } else {
                    bail!("unsupported OS")
                }
            }
        })
    }

    /// Direct URL to the release of an application for download.
    fn url(&self, version: &str) -> Result<String> {
        Ok(match self {
            Self::WasmBindgen => format!(
                "https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-{target}.tar.gz",
                version = version,
                target = self.target()?
            ),
            Self::WasmOpt => format!(
                "https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-x86_64-{target}.tar.gz",
                version = version,
                target = self.target()?,
            ),
        })
    }
}

/// Locate the given application and download it if missing.
#[tracing::instrument(level = "trace")]
pub async fn get(app: Application, version: Option<&str>) -> Result<PathBuf> {
    let version = version.unwrap_or_else(|| app.default_version());

    if let Some(path) = find_system(app, version).await {
        tracing::info!(app = app.name(), version = version, "using system installed binary");
        return Ok(path);
    }

    let cache_dir = cache_dir().await?;
    let app_dir = cache_dir.join(format!("{}-{}", app.name(), version));
    let bin_path = app_dir.join(app.path());

    if !is_executable(&bin_path).await? {
        let path = download(app, version)
            .await
            .context("failed downloading release archive")?;

        let mut file = File::open(&path).await.context("failed opening downloaded file")?;
        install(app, &mut file, &app_dir).await?;
        tokio::fs::remove_file(path)
            .await
            .context("failed deleting temporary archive")?;
    }

    Ok(bin_path)
}

/// Try to find a globally system installed version of the application and ensure it is the needed
/// release version.
#[tracing::instrument(level = "trace")]
async fn find_system(app: Application, version: &str) -> Option<PathBuf> {
    let result = || async {
        let path = which::which(app.name())?;
        let output = Command::new(&path).arg("--version").output().await?;

        ensure!(output.status.success(), "running command `{} --version` failed", path.display());

        let text = String::from_utf8_lossy(&output.stdout);
        let text = text.trim();

        let system_version = match app {
            Application::WasmBindgen => text.splitn(2, ' ').nth(1).context("missing version")?.to_owned(),
            Application::WasmOpt => text.splitn(2, ' ').nth(1).context("missing version")?.replace(' ', "_"),
        };

        Ok((path, system_version))
    };

    match result().await {
        Ok((path, system_version)) => (system_version == version).then(|| path),
        Err(e) => {
            tracing::debug!("system version not found for {}: {}", app.name(), e);
            None
        }
    }
}

/// Download a file from its remote location in the given version, extract it and make it ready for
/// execution at the given location.
#[tracing::instrument(level = "trace")]
async fn download(app: Application, version: &str) -> Result<PathBuf> {
    tracing::info!(version = version, "downloading {}", app.name());

    let cache_dir = cache_dir().await.context("failed getting the cache directory")?;
    let temp_out = cache_dir.join(format!("{}-{}.tmp", app.name(), version));
    let mut file = File::create(&temp_out)
        .await
        .context("failed creating temporary output file")?;

    let resp = reqwest::get(app.url(version)?)
        .await
        .context("error sending HTTP request")?;
    ensure!(
        resp.status().is_success(),
        "error downloading archive file: {:?}\n{}",
        resp.status(),
        app.url(version)?
    );
    let mut res_bytes = resp.bytes_stream();
    while let Some(chunk_res) = res_bytes.next().await {
        let chunk = chunk_res.context("error reading chunk from download")?;
        let _res = file.write(chunk.as_ref()).await;
    }

    Ok(temp_out)
}

/// Install an application from a downloaded archive locating and copying it to the given target
/// location.
#[tracing::instrument(level = "trace")]
async fn install(app: Application, archive_file: &mut File, target: &Path) -> Result<()> {
    tracing::info!("installing {}", app.name());

    let mut archive = Archive::new(GzipDecoder::new(BufReader::new(archive_file)));
    let mut file = extract_file(&mut archive, target, Path::new(app.path())).await?;

    set_executable_flag(&mut file).await?;

    for path in app.extra_paths() {
        // Archive must be opened for each entry as tar files don't allow jumping forth and back.
        let mut archive_file = archive
            .into_inner()
            .map_err(|_| anyhow!("error seeking app archive"))?
            .into_inner();
        archive_file
            .seek(SeekFrom::Start(0))
            .await
            .context("error seeking to beginning of archive")?;

        archive = Archive::new(GzipDecoder::new(archive_file));
        extract_file(&mut archive, target, Path::new(path)).await?;
    }

    Ok(())
}

/// Extract a single file from the given archive and put it into the target location.
async fn extract_file<R>(archive: &mut Archive<R>, target: &Path, file: &Path) -> Result<File>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let mut tar_file = find_tar_entry(archive, file).await?.context("file not found in archive")?;
    let out = target.join(file);

    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed creating output directory")?;
    }

    let mut out = File::create(target.join(file))
        .await
        .context("failed creating output file")?;
    tokio::io::copy(&mut tar_file, &mut out)
        .await
        .context("failed copying over final output file from archive")?;

    Ok(out)
}

/// Locate the cache dir for trunk and make sure it exists.
pub async fn cache_dir() -> Result<PathBuf> {
    let path = ProjectDirs::from("dev", "trunkrs", "trunk")
        .context("failed finding project directory")?
        .cache_dir()
        .to_owned();
    tokio::fs::create_dir_all(&path)
        .await
        .context("failed creating cache directory")?;
    Ok(path)
}

/// Set the executable flag for a file. Only has an effect on UNIX platforms.
async fn set_executable_flag(file: &mut File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = file.metadata().await.context("failed getting metadata")?.permissions();
        perms.set_mode(perms.mode() | 0o100);
        file.set_permissions(perms)
            .await
            .context("failed setting the executable flag")?;
    }

    Ok(())
}

/// Find an entry in a TAR archive by name and open it for reading. The first part of the path is
/// dropped as that's usually the folder name it was created from.
async fn find_tar_entry<R>(archive: &mut Archive<R>, path: impl AsRef<Path>) -> Result<Option<Entry<Archive<R>>>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let mut entries = archive.entries().context("failed getting archive entries")?;
    while let Some(entry) = entries.next().await {
        let entry = entry.context("error while getting archive entry")?;
        let name = entry.path().context("invalid entry path")?;

        let mut name = name.components();
        name.next();

        if name.as_path() == path.as_ref() {
            return Ok(Some(entry));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};

    #[tokio::test]
    async fn download_and_install_binaries() -> Result<()> {
        let dir = tempfile::tempdir().context("error creating temporary dir")?;

        for &app in &[Application::WasmBindgen, Application::WasmOpt] {
            let path = download(app, app.default_version())
                .await
                .context("error downloading app")?;
            let mut file = File::open(&path).await.context("error opening file")?;
            install(app, &mut file, dir.path()).await.context("error installing app")?;
            std::fs::remove_file(path).context("error during cleanup")?;
        }
        Ok(())
    }
}
