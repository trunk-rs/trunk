//! Download management for external tools and applications. Locate and automatically download
//! applications (if needed) to use them in the build pipeline.

use self::archive::Archive;
use crate::common::{is_executable, path_exists, path_exists_and};
use anyhow::{anyhow, bail, ensure, Context, Result};
use directories::ProjectDirs;
use futures_util::stream::StreamExt;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, OnceCell};

/// The application to locate and eventually download when calling [`get`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum Application {
    /// sass for generating css
    Sass,
    /// tailwindcss for generating css
    TailwindCss,
    /// tailwindcss-extra for generating css with DaisyUI bundled.
    TailwindCssExtra,
    /// wasm-bindgen for generating the JS bindings.
    WasmBindgen,
    /// wasm-opt to improve performance and size of the output file further.
    WasmOpt,
}

/// These options configure how Trunk sets up it's HTTP Client.
#[derive(Debug, Clone, Default)]
pub struct HttpClientOptions {
    /// Use this specific root certificate to validate the certificate chain. Optional.
    ///
    /// Useful when behind a corporate proxy that uses a self-signed root certificate.
    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    pub root_certificate: Option<PathBuf>,
    /// Allows Trunk to accept certificates that can't be verified when fetching dependencies. Defaults to false.
    ///
    /// **WARNING**: This is inherently unsafe and can open you up to Man-in-the-middle attacks. But sometimes it is required when working behind corporate proxies.
    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    pub accept_invalid_certificates: bool,
}

impl Application {
    /// Base name of the executable without extension.
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Sass => "sass",
            Self::TailwindCss => "tailwindcss",
            Self::TailwindCssExtra => "tailwindcss-extra",
            Self::WasmBindgen => "wasm-bindgen",
            Self::WasmOpt => "wasm-opt",
        }
    }

    /// Path of the executable within the downloaded archive.
    pub(crate) fn path(&self) -> &str {
        if cfg!(target_os = "windows") {
            match self {
                Self::Sass => "sass.bat",
                Self::TailwindCss => "tailwindcss.exe",
                Self::TailwindCssExtra => "tailwindcss-extra.exe",
                Self::WasmBindgen => "wasm-bindgen.exe",
                Self::WasmOpt => "bin/wasm-opt.exe",
            }
        } else {
            match self {
                Self::Sass => "sass",
                Self::TailwindCss => "tailwindcss",
                Self::TailwindCssExtra => "tailwindcss-extra",
                Self::WasmBindgen => "wasm-bindgen",
                Self::WasmOpt => "bin/wasm-opt",
            }
        }
    }

    /// Additional files included in the archive that are required to run the main binary.
    pub(crate) fn extra_paths(&self) -> &[&str] {
        match self {
            Self::Sass => {
                if cfg!(target_os = "windows") {
                    &["src/dart.exe", "src/sass.snapshot"]
                } else {
                    &["src/dart", "src/sass.snapshot"]
                }
            }
            Self::TailwindCss => &[],
            Self::TailwindCssExtra => &[],
            Self::WasmBindgen => &[],
            Self::WasmOpt => {
                if cfg!(target_os = "macos") {
                    &["lib/libbinaryen.dylib"]
                } else {
                    &[]
                }
            }
        }
    }

    /// Default version to use if not set by the user.
    pub(crate) fn default_version(&self) -> &str {
        match self {
            Self::Sass => "1.69.5",
            Self::TailwindCss => "3.3.5",
            Self::TailwindCssExtra => "1.7.25",
            Self::WasmBindgen => "0.2.89",
            Self::WasmOpt => "version_116",
        }
    }

    /// Direct URL to the release of an application for download.
    pub(crate) fn url(&self, version: &str) -> Result<String> {
        let target_os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            bail!("unsupported OS")
        };

        let target_arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            bail!("unsupported target architecture")
        };

        Ok(match self {
            Self::Sass => match (target_os, target_arch) {
                ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
                ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
                ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),
                _ => bail!("Unable to download Sass for {target_os} {target_arch}")
            },

            Self::TailwindCss => match (target_os, target_arch) {
                ("windows", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-windows-x64.exe"),
                ("macos" | "linux", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-{target_os}-x64"),
                ("macos" | "linux", "aarch64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-{target_os}-arm64"),
                _ => bail!("Unable to download tailwindcss for {target_os} {target_arch}")
            },

            Self::TailwindCssExtra => match (target_os, target_arch) {
                ("windows", "x86_64") => format!("https://github.com/dobicinaitis/tailwind-cli-extra/releases/download/v{version}/tailwindcss-extra-windows-x64.exe"),
                ("macos" | "linux", "x86_64") => format!("https://github.com/dobicinaitis/tailwind-cli-extra/releases/download/v{version}/tailwindcss-extra-{target_os}-x64"),
                ("macos" | "linux", "aarch64") => format!("https://github.com/dobicinaitis/tailwind-cli-extra/releases/download/v{version}/tailwindcss-extra-{target_os}-arm64"),
                _ => bail!("Unable to download tailwindcss for {target_os} {target_arch}")
            },

            Self::WasmBindgen => match (target_os, target_arch) {
                ("windows", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-pc-windows-msvc.tar.gz"),
                ("macos", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-apple-darwin.tar.gz"),
                ("macos", "aarch64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-aarch64-apple-darwin.tar.gz"),
                ("linux", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-unknown-linux-musl.tar.gz"),
                ("linux", "aarch64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-aarch64-unknown-linux-gnu.tar.gz"),
                _ => bail!("Unable to download wasm-bindgen for {target_os} {target_arch}")
            },

            Self::WasmOpt => match (target_os, target_arch) {
                ("macos", "aarch64") => format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-arm64-macos.tar.gz"),
                _ => format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target_arch}-{target_os}.tar.gz")
            }
        })
    }

    /// The CLI subcommand, flag or option used to check the application's version.
    fn version_test(&self) -> &'static str {
        match self {
            Application::Sass => "--version",
            Application::TailwindCss => "--help",
            Application::TailwindCssExtra => "--help",
            Application::WasmBindgen => "--version",
            Application::WasmOpt => "--version",
        }
    }

    /// Format the output of version checking the app.
    pub(crate) fn format_version_output(&self, text: &str) -> Result<String> {
        let text = text.trim();
        let formatted_version = match self {
            Application::Sass => text
                .split_whitespace()
                .next()
                .with_context(|| format!("missing or malformed version output: {}", text))?
                .to_owned(),
            Application::TailwindCss => text
                .lines()
                .find(|s| !str::is_empty(s))
                .and_then(|s| s.split(" v").nth(1))
                .with_context(|| format!("missing or malformed version output: {}", text))?
                .to_owned(),
            Application::TailwindCssExtra => text
                .lines()
                .find(|s| !str::is_empty(s))
                .and_then(|s| s.split(" v").nth(1))
                .with_context(|| format!("missing or malformed version output: {}", text))?
                .to_owned(),
            Application::WasmBindgen => text
                .split(' ')
                .nth(1)
                .with_context(|| format!("missing or malformed version output: {}", text))?
                .to_owned(),
            Application::WasmOpt => format!(
                "version_{}",
                text.split(' ')
                    .nth(2)
                    .with_context(|| format!("missing or malformed version output: {}", text))?
            ),
        };
        Ok(formatted_version)
    }
}

/// Global, application wide app cache that keeps track of what tools have already been
/// downloaded and installed to avoid duplicate installation runs.
static GLOBAL_APP_CACHE: Lazy<Mutex<AppCache>> = Lazy::new(|| Mutex::new(AppCache::new()));

/// An app cache that does the actual download and installation of tools while keeping track of
/// what has already been installed in the current trunk execution.
///
/// This cache doesn't keep track of any system-installed tools or the one's that have been
/// installed in previous runs of trunk. It only helps in avoiding a download of the same tool
/// concurrently during a single run of trunk.
struct AppCache(HashMap<(Application, String), OnceCell<()>>);

impl AppCache {
    /// Create a new app cache.
    fn new() -> Self {
        Self(HashMap::new())
    }

    /// Install the desired application of given version to the provided application directory. Or
    /// don't if it's already been installed.
    async fn install_once(
        &mut self,
        app: Application,
        version: &str,
        app_dir: PathBuf,
        client_options: &HttpClientOptions,
    ) -> Result<()> {
        let cached = self.0.entry((app, version.to_owned())).or_default();

        cached
            .get_or_try_init(|| async move {
                let path = download(app, version, client_options)
                    .await
                    .context("failed downloading release archive")?;

                let file = File::open(&path)
                    .await
                    .context("failed opening downloaded file")?;
                install(app, file, app_dir).await?;
                tokio::fs::remove_file(path)
                    .await
                    .context("failed deleting temporary archive")?;

                Ok(())
            })
            .await
            .map(|_| ())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolInformation {
    /// The path to the tool's binary
    pub path: PathBuf,
    /// The version of the tool
    pub version: String,
}

/// Locate the given application and download it if missing.
#[tracing::instrument(level = "debug")]
pub async fn get(
    app: Application,
    version: Option<&str>,
    offline: bool,
    client_options: &HttpClientOptions,
) -> Result<PathBuf> {
    Ok(get_info(app, version, offline, client_options).await?.path)
}

/// Locate the given application and download it if missing, returning detailed information.
#[tracing::instrument(level = "debug")]
pub async fn get_info(
    app: Application,
    version: Option<&str>,
    offline: bool,
    client_options: &HttpClientOptions,
) -> Result<ToolInformation> {
    tracing::debug!("Getting tool");

    if let Some((path, detected_version)) = find_system(app).await {
        // consider system installed version

        if let Some(required_version) = version {
            // we have a version requirement
            if required_version == detected_version {
                // and a match, so return early
                tracing::debug!(%detected_version, "using system installed binary: {}", path.display());
                return Ok(ToolInformation {
                    path,
                    version: detected_version,
                });
            } else if offline {
                // a mismatch, in offline mode, we can't help here
                bail!(
                    "couldn't find the required version ({required_version}) of the application {} (found: {detected_version}), unable to download in offline mode",
                    app.name(),
                )
            } else {
                // a mismatch, so we need to download
                tracing::debug!("tool version mismatch (required: {required_version}, system: {detected_version})");
            }
        } else {
            // we don't require any specific version
            return Ok(ToolInformation {
                path,
                version: detected_version,
            });
        }
    }

    if offline {
        return Err(anyhow!(
            "couldn't find application {name} (version: {version}), unable to download in offline mode",
            name = &app.name(),
            version = version.unwrap_or("<any>")
        ));
    }

    let cache_dir = cache_dir().await?;
    let version = version.unwrap_or_else(|| app.default_version());
    let app_dir = cache_dir.join(format!("{}-{}", app.name(), version));
    let bin_path = app_dir.join(app.path());

    if !is_executable(&bin_path).await? {
        GLOBAL_APP_CACHE
            .lock()
            .await
            .install_once(app, version, app_dir, client_options)
            .await?;
    }

    tracing::debug!(
        "Using {} ({version}) from: {}",
        app.name(),
        bin_path.display()
    );

    Ok(ToolInformation {
        path: bin_path,
        version: version.to_owned(),
    })
}

/// Try to find a global system installed version of the application.
#[tracing::instrument(level = "debug")]
pub async fn find_system(app: Application) -> Option<(PathBuf, String)> {
    // we wrap this into an fn to easier deal with result -> option conversion
    let result = || async {
        let path = which::which(app.name())?;
        let output = Command::new(&path).arg(app.version_test()).output().await?;
        ensure!(
            output.status.success(),
            "running command `{} {}` failed",
            path.display(),
            app.version_test()
        );

        let text = String::from_utf8_lossy(&output.stdout);
        let system_version = app.format_version_output(&text)?;

        tracing::debug!("system version found for {}: {system_version}", app.name());

        Ok((path, system_version))
    };

    match result().await {
        Ok(result) => Some(result),
        Err(err) => {
            tracing::debug!("failed to detect system tool: {err}");
            None
        }
    }
}

/// Download a file from its remote location in the given version, extract it and make it ready for
/// execution at the given location.
#[tracing::instrument(level = "trace")]
async fn download(
    app: Application,
    version: &str,
    client_options: &HttpClientOptions,
) -> Result<PathBuf> {
    tracing::info!(version = version, "downloading {}", app.name());

    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    if client_options.accept_invalid_certificates {
        tracing::warn!(
            "Accept Invalid Certificates is set to true. This can open you up to MITM attacks."
        );
    }

    let cache_dir = cache_dir()
        .await
        .context("failed getting the cache directory")?;
    let temp_out = cache_dir.join(format!("{}-{}.tmp", app.name(), version));
    let mut file = File::create(&temp_out)
        .await
        .context("failed creating temporary output file")?;

    let client = get_http_client(client_options).await?;

    let resp = client
        .get(app.url(version)?)
        .send()
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
async fn install(app: Application, archive_file: File, target_directory: PathBuf) -> Result<()> {
    tracing::info!("installing {}", app.name());

    let archive_file = archive_file.into_std().await;

    let target_directory_clone = target_directory.clone();
    tokio::task::spawn_blocking(move || {
        let mut archive = if app == Application::Sass && cfg!(target_os = "windows") {
            Archive::new_zip(archive_file)?
        } else if app == Application::TailwindCss {
            Archive::new_none(archive_file)
        } else {
            Archive::new_tar_gz(archive_file)
        };
        archive.extract_file(app.path(), &target_directory)?;

        for path in app.extra_paths() {
            // After extracting one file the archive must be reset.
            archive = archive.reset()?;
            if archive.extract_file(path, &target_directory).is_err() {
                tracing::warn!(
                    "attempted to extract '{}' from {:?} archive, but it is not present, this \
                     could be due to version updates",
                    path,
                    app
                );
            }
        }

        Result::<()>::Ok(())
    })
    .await
    .context("Unable to join on spawn_blocking")?
    .context("Could not extract files")?;

    let main_executable = target_directory_clone.join(app.path());
    let test = path_exists(&main_executable).await;
    ensure!(
        test.ok() == Some(true),
        "Extracted application binary {main_executable:?} could not be found."
    );

    let test = path_exists_and(&main_executable, |m| m.is_file()).await;
    ensure!(
        test.ok() == Some(true),
        "Extracted application binary {main_executable:?} is not a file"
    );

    let test = is_executable(&main_executable).await;
    ensure!(
        test.ok() == Some(true),
        "Extracted application binary {main_executable:?} is not executable."
    );

    Ok(())
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

async fn get_http_client(
    #[allow(unused_variables)] client_options: &HttpClientOptions,
) -> Result<reqwest::Client> {
    let builder = reqwest::ClientBuilder::new();

    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    let builder = {
        let mut builder =
            builder.danger_accept_invalid_certs(client_options.accept_invalid_certificates);

        if let Some(root_certs) = &client_options.root_certificate {
            let cert = tokio::fs::read(root_certs)
                .await
                .with_context(|| "Error reading certificate")
                .map_err(|err| {
                    crate::common::check_target_not_found_err(err, &root_certs.to_string_lossy())
                })?;

            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&cert)
                    .with_context(|| "Error adding root certificate")?,
            );
        }

        builder
    };

    builder
        .build()
        .with_context(|| "Error building http client")
}

mod archive {
    use std::fmt::Display;
    use std::fs::{self, File};
    use std::io::{self, BufReader, BufWriter, Read, Seek};
    use std::path::Path;

    use anyhow::{Context, Result};
    use flate2::read::GzDecoder;
    use tar::{Archive as TarArchive, Entry as TarEntry};
    use zip::ZipArchive;

    pub enum Archive {
        TarGz(Box<TarArchive<GzDecoder<BufReader<File>>>>),
        Zip(ZipArchive<BufReader<File>>),
        None(File),
    }

    impl Archive {
        pub fn new_tar_gz(file: File) -> Self {
            Self::TarGz(Box::new(TarArchive::new(GzDecoder::new(BufReader::new(
                file,
            )))))
        }

        pub fn new_zip(file: File) -> Result<Self> {
            Ok(Self::Zip(ZipArchive::new(BufReader::new(file))?))
        }

        pub fn new_none(file: File) -> Self {
            Self::None(file)
        }

        pub fn extract_file(&mut self, file: &str, target_directory: &Path) -> Result<()> {
            match self {
                Self::TarGz(archive) => {
                    let mut tar_file =
                        find_tar_entry(archive, file)?.context("file not found in archive")?;
                    let mut out_file = extract_file(&mut tar_file, file, target_directory)?;

                    if let Ok(mode) = tar_file.header().mode() {
                        set_file_permissions(&mut out_file, mode, file)?;
                    }
                }
                Self::Zip(archive) => {
                    let zip_index =
                        find_zip_entry(archive, file)?.context("file not found in archive")?;
                    let mut zip_file = archive.by_index(zip_index)?;
                    let mut out_file = extract_file(&mut zip_file, file, target_directory)?;

                    if let Some(mode) = zip_file.unix_mode() {
                        set_file_permissions(&mut out_file, mode, file)?;
                    }
                }
                Self::None(in_file) => {
                    std::fs::create_dir_all(target_directory).context("failed to open file for")?;

                    let mut out_file_path = target_directory.to_path_buf();
                    out_file_path.push(file);
                    let mut out_file =
                        File::create(&out_file_path).context("failed to open binary to copy")?;
                    {
                        let mut reader = BufReader::new(in_file);
                        let mut writer = BufWriter::new(&out_file);

                        std::io::copy(&mut reader, &mut writer).context("failed to copy binary")?;
                    }
                    set_file_permissions(&mut out_file, 0o755, out_file_path.display())?;
                }
            }

            Ok(())
        }

        pub fn reset(self) -> Result<Self> {
            match self {
                Self::TarGz(archive) => {
                    let mut archive_file = archive.into_inner().into_inner();
                    archive_file
                        .rewind()
                        .context("error seeking to beginning of archive")?;

                    Ok(Self::TarGz(Box::new(TarArchive::new(GzDecoder::new(
                        archive_file,
                    )))))
                }
                result @ Self::None(_) | result @ Self::Zip(_) => Ok(result),
            }
        }
    }

    /// Find an entry in a TAR archive by name and open it for reading. The first part of the path
    /// is dropped as that's usually the folder name it was created from.
    fn find_tar_entry(
        archive: &mut TarArchive<impl Read>,
        path: impl AsRef<Path>,
    ) -> Result<Option<TarEntry<impl Read>>> {
        let entries = archive
            .entries()
            .context("failed getting archive entries")?;
        for entry in entries {
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

    /// Find an entry in a ZIP archive by name and return its index. The first part of the path is
    /// dropped as that's usually the folder name it was created from.
    fn find_zip_entry(
        archive: &mut ZipArchive<impl Read + Seek>,
        path: impl AsRef<Path>,
    ) -> Result<Option<usize>> {
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .context("error while getting archive entry")?;
            let name = entry.enclosed_name().context("invalid entry path")?;

            let mut name = name.components();
            name.next();

            if name.as_path() == path.as_ref() {
                return Ok(Some(index));
            }
        }

        Ok(None)
    }

    fn extract_file(mut read: impl Read, file: &str, target_directory: &Path) -> Result<File> {
        let out = target_directory.join(file);

        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).context("failed creating output directory")?;
        }

        let mut out =
            File::create(target_directory.join(file)).context("failed creating output file")?;
        io::copy(&mut read, &mut out)
            .context("failed copying over final output file from archive")?;

        Ok(out)
    }

    /// Set the executable flag for a file. Only has an effect on UNIX platforms.
    #[cfg(not(unix))]
    fn set_file_permissions(
        _file: &mut File,
        _mode: u32,
        _file_path_hint: impl Display,
    ) -> Result<()> {
        Ok(())
    }

    /// Set the executable flag for a file. Only has an effect on UNIX platforms.
    #[cfg(unix)]
    fn set_file_permissions(
        file: &mut File,
        mode: u32,
        file_path_hint: impl Display,
    ) -> Result<()> {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        tracing::debug!("Setting permission of '{file_path_hint}' to {mode:#o}");

        file.set_permissions(Permissions::from_mode(mode))
            .context("failed setting file permissions")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn download_and_install_binaries() -> Result<()> {
        let dir = tempfile::tempdir().context("error creating temporary dir")?;

        for &app in &[
            Application::Sass,
            Application::WasmBindgen,
            Application::WasmOpt,
            Application::TailwindCss,
        ] {
            let path = download(app, app.default_version(), &HttpClientOptions::default())
                .await
                .context("error downloading app")?;
            let file = File::open(&path).await.context("error opening file")?;
            install(app, file, dir.path().to_owned())
                .await
                .context("error installing app")?;
            std::fs::remove_file(path).context("error during cleanup")?;
        }
        Ok(())
    }

    macro_rules! table_test_format_version {
        ($name:ident, $app:expr, $input:literal, $expect:literal) => {
            #[test]
            fn $name() -> Result<()> {
                let app = $app;
                let output = app
                    .format_version_output($input)
                    .context("unexpected version formatting error")?;
                ensure!(
                    output == $expect,
                    "version check output does not match: {} != {}",
                    $expect,
                    output
                );
                Ok(())
            }
        };
    }

    table_test_format_version!(
        wasm_opt_from_source,
        Application::WasmOpt,
        "wasm-opt version 101 (version_101)",
        "version_101"
    );

    table_test_format_version!(
        wasm_opt_pre_compiled,
        Application::WasmOpt,
        "wasm-opt version 101",
        "version_101"
    );

    table_test_format_version!(
        wasm_bindgen_from_source,
        Application::WasmBindgen,
        "wasm-bindgen 0.2.75",
        "0.2.75"
    );

    table_test_format_version!(
        wasm_bindgen_pre_compiled,
        Application::WasmBindgen,
        "wasm-bindgen 0.2.74 (27c7a4d06)",
        "0.2.74"
    );

    table_test_format_version!(sass_pre_compiled, Application::Sass, "1.37.5", "1.37.5");
    table_test_format_version!(
        sass_pre_compiled_dart2js,
        Application::Sass,
        "1.37.5 compiled with dart2js 2.18.4",
        "1.37.5"
    );
    table_test_format_version!(
        tailwindcss_pre_compiled,
        Application::TailwindCss,
        "tailwindcss v3.3.2",
        "3.3.2"
    );
    table_test_format_version!(
        tailwindcss_extra_pre_compiled,
        Application::TailwindCssExtra,
        "tailwindcss-extra v1.7.25",
        "1.7.25"
    );
}
