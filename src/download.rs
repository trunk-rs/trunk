//! Download management for external application. Locate and automatically downloading applications
//! if needed to use them in the build pipeline.

use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{bail, ensure, Context, Result};
use async_process::Command;
use async_std::{fs::File as AsyncFile, io};
use directories_next::ProjectDirs;
use flate2::read::GzDecoder;
use tar::Archive as TarArchive;
use zip::ZipArchive;

/// The application to locate and eventually download when calling [`binary`].
#[derive(Clone, Copy)]
pub enum Application {
    Sass,
    WasmBindgen,
    WasmOpt,
}

impl Application {
    /// Base name of the executable without extension.
    fn name(&self) -> &str {
        match self {
            Self::Sass => "sass",
            Self::WasmBindgen => "wasm-bindgen",
            Self::WasmOpt => "wasm-opt",
        }
    }

    fn path(&self) -> &str {
        if cfg!(windows) {
            match self {
                Self::Sass => "sass.bat",
                Self::WasmBindgen => "wasm-bindgen.exe",
                Self::WasmOpt => "bin/wasm-opt.exe",
            }
        } else {
            match self {
                Self::Sass | Self::WasmBindgen => self.name(),
                Self::WasmOpt => "bin/wasm-opt",
            }
        }
    }

    /// Default version to use if not set by the user.
    fn default_version(&self) -> &str {
        match self {
            Self::Sass => "1.32.8",
            Self::WasmBindgen => "0.2.71",
            Self::WasmOpt => "version_100",
        }
    }

    /// Target for the current OS as part of the download URL. Can fail as there might be no release
    /// for the current platform.
    fn target(&self) -> Result<&str> {
        Ok(match self {
            Self::Sass => {
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
            Self::Sass => format!(
                "https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target}-x64.{ext}",
                version = version,
                target = self.target()?,
                ext = if cfg!(windows) { "zip" } else { "tar.gz" }
            ),
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

    /// Take the release archive as it was download without any modification and extract the binary.
    fn unpack(&self, file: &File, target: &Path) -> Result<()> {
        match self {
            Self::Sass => {
                if cfg!(windows) {
                    let mut archive = ZipArchive::new(file)?;
                    let names = &["sass.bat", "src/dart.exe", "src/sass.snapshot"];

                    for name in names {
                        let mut file = find_zip_entry(&mut archive, Path::new(name))?.context("file not found in archive")?;
                        let out = target.join(name);

                        if let Some(parent) = out.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        let mut out = File::create(out)?;
                        std::io::copy(&mut file, &mut out)?;

                        drop(file);
                        archive = reset_zip_archive(archive)?;
                    }
                } else {
                    let mut archive = TarArchive::new(GzDecoder::new(file));
                    #[cfg(not(target_os = "linux"))]
                    let names = &[("sass", true), ("src/dart", true), ("src/sass.snapshot", false)];
                    // Somehow for all OSs sass is split into three files and only for linux it's a
                    // single binary instead.
                    #[cfg(target_os = "linux")]
                    let names = &[("sass", true)];

                    for (name, exec) in names.iter().copied() {
                        let mut file = find_tar_entry(&mut archive, Path::new(name))?.context("file not found in archive")?;
                        let out = target.join(name);

                        if let Some(parent) = out.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        let mut out = File::create(out)?;
                        std::io::copy(&mut file, &mut out)?;

                        drop(file);
                        archive = reset_tar_archive(archive)?;

                        if exec {
                            set_executable_flag(&mut out)?;
                        }
                    }
                }
            }
            Self::WasmBindgen | Self::WasmOpt => {
                let name = match self {
                    Self::WasmBindgen => OsStr::new(if cfg!(windows) { "wasm-bindgen.exe" } else { "wasm-bindgen" }),
                    Self::WasmOpt => OsStr::new(if cfg!(windows) { "bin/wasm-opt.exe" } else { "bin/wasm-opt" }),
                    _ => unreachable!(),
                };

                let mut archive = TarArchive::new(GzDecoder::new(file));
                let mut file = find_tar_entry(&mut archive, name)?.context("file not found in archive")?;
                let out = target.join(name);

                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut out = File::create(target.join(name))?;
                std::io::copy(&mut file, &mut out)?;

                set_executable_flag(&mut out)?;
            }
        }

        Ok(())
    }
}

/// Locate the given application and download it if missing.
pub async fn binary(app: Application, version: Option<&str>) -> Result<PathBuf> {
    let version = version.unwrap_or_else(|| app.default_version());

    if let Ok(path) = find_system(app, version).await {
        tracing::info!(app = app.name(), version = version, "using system installed binary");
        return Ok(path);
    }

    let cache_dir = cache_dir()?;
    let app_path = cache_dir.join(format!("{}-{}", app.name(), version));
    let bin_path = app_path.join(app.path());

    if !is_executable(&bin_path)? {
        download(app, version, &app_path).await?;
    }

    Ok(bin_path)
}

async fn find_system(app: Application, version: &str) -> Result<PathBuf> {
    let path = which::which(app.name())?;
    let output = Command::new(&path).arg("--version").output().await?;

    ensure!(output.status.success(), "running command failed");

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();

    let system_version = match app {
        Application::Sass => text.to_owned(),
        Application::WasmBindgen => text.splitn(2, ' ').nth(1).context("missing version")?.to_owned(),
        Application::WasmOpt => text.splitn(2, ' ').nth(1).context("missing version")?.replace(' ', "_"),
    };

    if system_version == version {
        Ok(path)
    } else {
        bail!("not found")
    }
}

/// Check whether a given path exists, is a file and marked as executable (unix only).
fn is_executable(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let metadata = path.metadata()?;
    if !metadata.is_file() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o100 == 0 {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Download a file from its remote location in the given version, extract it and make it ready for
// execution at the given location.
async fn download(app: Application, version: &str, target: &Path) -> Result<()> {
    tracing::info!(version = version, "downloading {}", app.name());

    let cache_dir = cache_dir()?;
    let client = surf::client().with(surf::middleware::Redirect::new(5));

    let mut resp = client.get(app.url(version)?).send().await.map_err(|e| e.into_inner())?;
    ensure!(
        resp.status().is_success(),
        "error downloading binary file: {:?}\n{}",
        resp.status(),
        app.url(version)?
    );

    let temp_out = cache_dir.join(format!("{}-{}.tmp", app.name(), version));
    let mut file = AsyncFile::create(&temp_out).await?;

    io::copy(resp.take_body().into_reader(), &mut file).await?;
    drop(file);

    app.unpack(&File::open(&temp_out)?, &target)?;
    std::fs::remove_file(temp_out)?;

    Ok(())
}

/// Locate the cache dir for this application and make sure it exists.
fn cache_dir() -> Result<PathBuf> {
    let path = ProjectDirs::from("dev", "trunkrs", "trunk")
        .context("failed finding project directory")?
        .cache_dir()
        .to_owned();
    std::fs::create_dir_all(&path)?;

    Ok(path)
}

/// Set the executable flag for a file. Only has an effect on UNIX platforms.
fn set_executable_flag(file: &mut File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(perms.mode() | 0o100);
        file.set_permissions(perms)?;
    }

    Ok(())
}

/// Find an entry in a ZIP archive by name and open it for reading. The first part of the path is
/// dropped as that's usually the folder name it was created from.
fn find_zip_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, path: impl AsRef<Path>) -> Result<Option<impl Read + '_>> {
    let name = archive
        .file_names()
        .find(|name| {
            let mut name = Path::new(name).components();
            name.next();
            name.as_path() == path.as_ref()
        })
        .map(str::to_owned);

    name.map(move |n| archive.by_name(&n)).transpose().map_err(Into::into)
}

/// Reset a ZIP archive after reading an entry. This is a no-op and just exists to have the same API
/// as TAR archives.
#[allow(clippy::unnecessary_wraps)]
fn reset_zip_archive<R: Read + Seek>(archive: ZipArchive<R>) -> Result<ZipArchive<R>> {
    Ok(archive)
}

/// Find an entry in a TAR archive by name and open it for reading. The first part of the path is
/// dropped as that's usually the folder name it was created from.
fn find_tar_entry<R: Read>(archive: &mut TarArchive<R>, path: impl AsRef<Path>) -> Result<Option<impl Read + '_>> {
    for entry in archive.entries()? {
        let entry = entry?;
        let name = entry.path()?;

        let mut name = name.components();
        name.next();

        if name.as_path() == path.as_ref() {
            return Ok(Some(entry));
        }
    }

    Ok(None)
}

/// Reset a TAR archive after reading an entry. This means to reset the inner file to the beginning
/// and then wrap it again with gzip and tar decoders.
fn reset_tar_archive<R: Read + Seek>(archive: TarArchive<GzDecoder<R>>) -> Result<TarArchive<GzDecoder<R>>> {
    let mut inner = archive.into_inner().into_inner();
    inner.seek(SeekFrom::Start(0))?;

    Ok(TarArchive::new(GzDecoder::new(inner)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn install_binaries() {
        binary(Application::Sass, None).await.unwrap();
        binary(Application::WasmBindgen, None).await.unwrap();
        binary(Application::WasmOpt, None).await.unwrap();
    }
}
