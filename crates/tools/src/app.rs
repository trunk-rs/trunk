//! Download management for external tools and applications. Locate and automatically download
//! applications (if needed) to use them in the build pipeline.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

// use anyhow::{bail, ensure, Context, Result};
use directories::ProjectDirs;
use futures_util::stream::StreamExt;
use once_cell::sync::Lazy;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, OnceCell};
use trunk_util::Executable;

use crate::util::{is_executable, Archive, ErrorReason, Result, ResultExt};

fn parse_sass_version(text: &str) -> Result<Cow<'static, str>> {
    let text = text.trim();
    let formatted_version = text
        .lines()
        .next()
        .with_reason(|| ErrorReason::ToolchainMalformedVersion(text.to_owned()))?
        .to_owned();

    Ok(formatted_version.into())
}

fn get_sass_url(version: &str) -> Result<Cow<'static, str>> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let url = match (target_os, target_arch) {
              ("windows", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-windows-x64.zip"),
              ("macos" | "linux", "x86_64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-x64.tar.gz"),
              ("macos" | "linux", "aarch64") => format!("https://github.com/sass/dart-sass/releases/download/{version}/dart-sass-{version}-{target_os}-arm64.tar.gz"),

              _ => return Err(ErrorReason::ToolchainUnsupportedTarget.into_error()),
        };

    Ok(url.into())
}

fn parse_tailwind_css_version(text: &str) -> Result<Cow<'static, str>> {
    let text = text.trim();

    let formatted_version = text
        .lines()
        .find(|s| !str::is_empty(s))
        .and_then(|s| s.split(" v").nth(1))
        .with_reason(|| ErrorReason::ToolchainMalformedVersion(text.to_owned()))?
        .to_owned();

    Ok(formatted_version.into())
}

fn get_tailwind_css_url(version: &str) -> Result<Cow<'static, str>> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let url = match (target_os, target_arch) {
                ("windows", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-windows-x64.exe"),
                ("macos" | "linux", "x86_64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-{target_os}-x64"),
                ("macos" | "linux", "aarch64") => format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}/tailwindcss-{target_os}-arm64"),
              _ => return Err(ErrorReason::ToolchainUnsupportedTarget.into_error()),
            };

    Ok(url.into())
}

fn parse_wasm_bindgen_version(text: &str) -> Result<Cow<'static, str>> {
    let text = text.trim();

    let formatted_version = text
        .split(' ')
        .nth(1)
        .with_reason(|| ErrorReason::ToolchainMalformedVersion(text.to_owned()))?
        .to_owned();

    Ok(formatted_version.into())
}

fn get_wasm_bindgen_url(version: &str) -> Result<Cow<'static, str>> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let url = match (target_os, target_arch) {
        ("windows", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-pc-windows-msvc.tar.gz"),
              ("macos", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-apple-darwin.tar.gz"),
              ("macos", "aarch64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-aarch64-apple-darwin.tar.gz"),
              ("linux", "x86_64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-unknown-linux-musl.tar.gz"),
              ("linux", "aarch64") => format!("https://github.com/rustwasm/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-aarch64-unknown-linux-gnu.tar.gz"),

          _ => return Err(ErrorReason::ToolchainUnsupportedTarget.into_error()),
        };

    Ok(url.into())
}

fn parse_wasm_opt_version(text: &str) -> Result<Cow<'static, str>> {
    let text = text.trim();
    let formatted_version = format!(
        "version_{}",
        text.split(' ')
            .nth(2)
            .with_reason(|| ErrorReason::ToolchainMalformedVersion(text.to_owned()))?
    );

    Ok(formatted_version.into())
}

fn get_wasm_opt_url(version: &str) -> Result<Cow<'static, str>> {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(ErrorReason::ToolchainUnsupportedTarget.into_error());
    };

    let url = match (target_os, target_arch) {
        ("macos", "aarch64") => format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-arm64-macos.tar.gz"),
              _ => format!("https://github.com/WebAssembly/binaryen/releases/download/{version}/binaryen-{version}-{target_arch}-{target_os}.tar.gz")


        };

    Ok(url.into())
}

/// The application to locate and eventually download when calling [`get`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Application {
    pub name: Cow<'static, str>,
    pub executable_path: Cow<'static, str>,
    pub extra_paths: Cow<'static, [Cow<'static, str>]>,
    pub default_version: Cow<'static, str>,
    pub version_arg: Cow<'static, str>,
    pub parse_version: fn(&str) -> Result<Cow<'static, str>>,
    pub get_url: fn(version: &str) -> Result<Cow<'static, str>>,
}

impl Application {
    /// sass for generating css
    pub const SASS: Self = Self {
        name: Cow::Borrowed("sass"),

        #[cfg(not(windows))]
        executable_path: Cow::Borrowed("sass"),
        #[cfg(windows)]
        executable_path: Cow::Borrowed("sass.bat"),
        #[cfg(not(windows))]
        extra_paths: Cow::Borrowed(&[
            Cow::Borrowed("src/dart"),
            Cow::Borrowed("src/sass.snapshot"),
        ]),
        #[cfg(windows)]
        extra_paths: Cow::Borrowed(&[
            Cow::Borrowed("src/dart.exe"),
            Cow::Borrowed("src/sass.snapshot"),
        ]),

        default_version: Cow::Borrowed("1.63.6"),
        version_arg: Cow::Borrowed("--version"),
        parse_version: parse_sass_version,
        get_url: get_sass_url,
    };
    /// tailwindcss for generating css
    pub const TAILWIND_CSS: Self = Self {
        name: Cow::Borrowed("tailwindcss"),

        #[cfg(not(windows))]
        executable_path: Cow::Borrowed("tailwindcss"),
        #[cfg(windows)]
        executable_path: Cow::Borrowed("tailwindcss.exe"),
        extra_paths: Cow::Borrowed(&[]),

        default_version: Cow::Borrowed("3.3.2"),
        version_arg: Cow::Borrowed("--help"),
        parse_version: parse_tailwind_css_version,
        get_url: get_tailwind_css_url,
    };
    /// wasm-bindgen for generating the JS bindings.
    pub const WASM_BINDGEN: Self = Self {
        name: Cow::Borrowed("wasm-bindgen"),

        #[cfg(not(windows))]
        executable_path: Cow::Borrowed("wasm-bindgen"),
        #[cfg(windows)]
        executable_path: Cow::Borrowed("wasm-bindgen.exe"),
        extra_paths: Cow::Borrowed(&[]),

        default_version: Cow::Borrowed("0.2.87"),
        version_arg: Cow::Borrowed("--version"),
        parse_version: parse_wasm_bindgen_version,
        get_url: get_wasm_bindgen_url,
    };
    /// wasm-opt to improve performance and size of the output file further.
    pub const WASM_OPT: Self = Self {
        name: Cow::Borrowed("wasm-opt"),

        #[cfg(not(windows))]
        executable_path: Cow::Borrowed("bin/wasm-opt"),
        #[cfg(windows)]
        executable_path: Cow::Borrowed("bin/wasm-opt.exe"),
        #[cfg(target_os = "macos")]
        extra_paths: Cow::Borrowed(&[Cow::Borrowed("lib/libbinaryen.dylib")]),
        #[cfg(not(target_os = "macos"))]
        extra_paths: Cow::Borrowed(&[]),

        default_version: Cow::Borrowed("version_113"),
        version_arg: Cow::Borrowed("--version"),
        parse_version: parse_wasm_opt_version,
        get_url: get_wasm_opt_url,
    };

    /// Base name of the executable without extension.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Path of the executable within the downloaded archive.
    fn path(&self) -> &str {
        &self.executable_path
    }

    /// Additional files included in the archive that are required to run the main binary.
    fn extra_paths(&self) -> impl '_ + Iterator<Item = impl '_ + AsRef<str>> {
        self.extra_paths.iter()
    }

    /// Default version to use if not set by the user.
    fn default_version(&self) -> &str {
        &self.default_version
    }

    /// Direct URL to the release of an application for download.
    fn url(&self, version: &str) -> Result<Cow<'static, str>> {
        (self.get_url)(version)
    }

    /// The CLI subcommand, flag or option used to check the application's version.
    fn version_test(&self) -> &str {
        self.version_arg.as_ref()
    }

    /// Format the output of version checking the app.
    fn format_version_output(&self, text: &str) -> Result<Cow<'static, str>> {
        (self.parse_version)(text)
    }

    /// Locate the given application and download it if missing.
    #[tracing::instrument(level = "trace")]
    pub async fn get(&self, version: Option<&str>) -> Result<Executable> {
        if let Some((path, version)) = find_system(self, version).await {
            tracing::info!(app = %self.name(), %version, "using system installed binary");
            return Ok(Executable::new(path).with_name(self.name().to_owned()));
        }

        let cache_dir = cache_dir().await?;
        let version = version
            .map(|s| s.to_owned())
            .unwrap_or_else(|| self.default_version().to_owned());
        let app_dir = cache_dir.join(format!("{}-{}", self.name(), version));
        let bin_path = app_dir.join(self.path());

        if !is_executable(&bin_path).await? {
            GLOBAL_APP_CACHE
                .lock()
                .await
                .install_once(self, version.as_str(), app_dir)
                .await?;
        }

        let exec = Executable::new(bin_path).with_name(self.name().to_owned());

        Ok(exec)
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
        app: &Application,
        version: &str,
        app_dir: PathBuf,
    ) -> Result<()> {
        let cached = self
            .0
            .entry((app.clone(), version.to_owned()))
            .or_insert_with(OnceCell::new);

        cached
            .get_or_try_init(|| async move {
                let path = download(app, version)
                    .await
                    .reason(ErrorReason::ToolchainDownloadFailed)?;

                let file = File::open(&path)
                    .await
                    .reason(ErrorReason::ToolchainOpenFailed)?;
                install(app, file, app_dir).await?;
                tokio::fs::remove_file(path)
                    .await
                    .reason(ErrorReason::ToolchainDeleteFailed)?;

                Ok(())
            })
            .await
            .map(|_| ())
    }
}

/// Try to find a globally system installed version of the application and ensure it is the needed
/// release version.
#[tracing::instrument(level = "trace")]
async fn find_system(
    app: &Application,
    version: Option<&str>,
) -> Option<(PathBuf, Cow<'static, str>)> {
    let result = || async {
        let path = which::which(app.name()).reason(ErrorReason::ToolchainFileNotFound)?;
        let output = Command::new(&path)
            .arg(app.version_test())
            .output()
            .await
            .with_reason(|| {
                ErrorReason::ToolchainCommandFailed(format!(
                    "{} {}",
                    path.display(),
                    app.version_test()
                ))
            })?;

        if !output.status.success() {
            return Err(ErrorReason::ToolchainCommandFailed(format!(
                "{} {}",
                path.display(),
                app.version_test()
            ))
            .into_error());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let system_version = app.format_version_output(&text)?;

        Ok((path, system_version))
    };

    match result().await {
        Ok((path, system_version)) => version
            .map(|v| v == system_version)
            .unwrap_or(true)
            .then_some((path, system_version)),
        Err(e) => {
            tracing::debug!("system version not found for {}: {}", app.name(), e);
            None
        }
    }
}

/// Locate the cache dir for trunk and make sure it exists.
pub async fn cache_dir() -> Result<PathBuf> {
    let path = ProjectDirs::from("dev", "trunkrs", "trunk")
        .reason(ErrorReason::ToolchainCreateCacheFailed)?
        .cache_dir()
        .to_owned();
    tokio::fs::create_dir_all(&path)
        .await
        .reason(ErrorReason::ToolchainCreateCacheFailed)?;
    Ok(path)
}

/// Download a file from its remote location in the given version, extract it and make it ready for
/// execution at the given location.
#[tracing::instrument(level = "trace")]
async fn download(app: &Application, version: &str) -> Result<PathBuf> {
    tracing::info!(version = version, "downloading {}", app.name());

    let cache_dir = cache_dir()
        .await
        .reason(ErrorReason::ToolchainCreateCacheFailed)?;
    let temp_out = cache_dir.join(format!("{}-{}.tmp", app.name(), version));
    let mut file = File::create(&temp_out)
        .await
        .reason(ErrorReason::ToolchainWriteFailed)?;

    let resp = reqwest::get(app.url(version)?.as_ref())
        .await
        .and_then(|m| m.error_for_status())
        .reason(ErrorReason::ToolchainDownloadFailed)?;

    let mut res_bytes = resp.bytes_stream();
    while let Some(chunk_res) = res_bytes.next().await {
        let chunk = chunk_res.reason(ErrorReason::ToolchainDownloadFailed)?;
        let _res = file.write(chunk.as_ref()).await;
    }

    Ok(temp_out)
}

/// Install an application from a downloaded archive locating and copying it to the given target
/// location.
#[tracing::instrument(level = "trace")]
async fn install(app: &Application, archive_file: File, target: PathBuf) -> Result<()> {
    tracing::info!("installing {}", app.name());

    let archive_file = archive_file.into_std().await;

    let app = app.clone();
    tokio::task::spawn_blocking(move || {
        let mut archive = if app == Application::SASS && cfg!(target_os = "windows") {
            Archive::new_zip(archive_file)?
        } else if app == Application::TAILWIND_CSS {
            Archive::new_none(archive_file)
        } else {
            Archive::new_tar_gz(archive_file)
        };
        archive.extract_file(app.path(), &target)?;

        for path in app.extra_paths() {
            let path = path.as_ref();
            // After extracting one file the archive must be reset.
            archive = archive.reset()?;
            if archive.extract_file(path, &target).is_err() {
                tracing::warn!(
                    "attempted to extract '{}' from {:?} archive, but it is not present, this \
                     could be due to version updates",
                    path,
                    app
                );
            }
        }

        Ok(())
    })
    .await
    .reason(ErrorReason::TokioTaskFailed)?
}

#[cfg(test)]
mod tests {
    use anyhow::{ensure, Context, Result};

    use super::*;

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn download_and_install_binaries() -> Result<()> {
        let dir = tempfile::tempdir().context("error creating temporary dir")?;

        for app in [
            Application::SASS,
            Application::WASM_BINDGEN,
            Application::WASM_OPT,
            Application::TAILWIND_CSS,
        ]
        .iter()
        {
            let path = download(app, app.default_version())
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
        Application::WASM_OPT,
        "wasm-opt version 101 (version_101)",
        "version_101"
    );

    table_test_format_version!(
        wasm_opt_pre_compiled,
        Application::WASM_OPT,
        "wasm-opt version 101",
        "version_101"
    );

    table_test_format_version!(
        wasm_bindgen_from_source,
        Application::WASM_BINDGEN,
        "wasm-bindgen 0.2.75",
        "0.2.75"
    );

    table_test_format_version!(
        wasm_bindgen_pre_compiled,
        Application::WASM_BINDGEN,
        "wasm-bindgen 0.2.74 (27c7a4d06)",
        "0.2.74"
    );

    table_test_format_version!(sass_pre_compiled, Application::SASS, "1.37.5", "1.37.5");
    table_test_format_version!(
        tailwindcss_pre_compiled,
        Application::TAILWIND_CSS,
        "tailwindcss v3.3.2",
        "3.3.2"
    );
}
