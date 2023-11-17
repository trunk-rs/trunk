use crate::config::{
    ConfigOptsBuild, ConfigOptsClean, ConfigOptsHook, ConfigOptsProxy, ConfigOptsServe,
    ConfigOptsTools, ConfigOptsWatch, WsProtocol,
};
use anyhow::{anyhow, ensure, Context, Result};
use axum::http::Uri;
use axum_server::tls_rustls::RustlsConfig;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Config options for the cargo build command
#[derive(Clone, Debug)]
pub enum Features {
    /// Use cargo's `--all-features` flag during compilation.
    All,
    /// An explicit list of features to use; might be empty; might include no-default-features.
    Custom {
        /// Space or comma separated list of cargo features to activate.
        features: Option<String>,
        /// Use cargo's `--no-default-features` flag during compilation.
        no_default_features: bool,
    },
}

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// The parent directory of the target index HTML file.
    pub target_parent: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// Build without network access
    pub offline: bool,
    /// Require Cargo.lock and cache are up to date
    pub frozen: bool,
    /// Require Cargo.lock is up to date
    pub locked: bool,
    /// The public URL from which assets are to be served.
    pub public_url: String,
    /// If `true`, then files being processed should be hashed and the hash should be
    /// appended to the file's name.
    pub filehash: bool,
    /// The directory where final build artifacts are placed after a successful build.
    pub final_dist: PathBuf,
    /// The directory used to stage build artifacts during an active build.
    pub staging_dist: PathBuf,
    /// The configuration of the features passed to cargo.
    pub cargo_features: Features,
    /// Configuration for automatic application download.
    pub tools: ConfigOptsTools,
    /// Build process hooks.
    pub hooks: Vec<ConfigOptsHook>,
    /// A bool indicating if the output HTML should have the WebSocket autoloader injected.
    ///
    /// This value is configured via the server config only. If the server is not being used, then
    /// the autoloader will not be injected.
    pub inject_autoloader: bool,
    /// A bool indication if the output HTML should have module preloads and scripts injected.
    pub inject_scripts: bool,
    /// Optional pattern for the app loader script.
    pub pattern_script: Option<String>,
    /// Optional pattern for the app preload element.
    pub pattern_preload: Option<String>,
    /// Optional replacement parameters corresponding to the patterns provided in
    /// `pattern_script` and `pattern_preload`.
    pub pattern_params: Option<HashMap<String, String>>,
}

impl RtcBuild {
    /// Construct a new instance.
    pub(super) fn new(
        opts: ConfigOptsBuild,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        inject_autoloader: bool,
    ) -> Result<Self> {
        // Get the canonical path to the target HTML file.
        let pre_target = opts.target.clone().unwrap_or_else(|| "index.html".into());
        let target = pre_target.canonicalize().with_context(|| {
            format!(
                "error getting canonical path to source HTML file {:?}",
                &pre_target
            )
        })?;

        // Get the target HTML's parent dir, falling back to OS specific root, as that is the only
        // time where no parent could be determined.
        let target_parent = target
            .parent()
            .map(|path| path.to_owned())
            .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));

        // Ensure the final dist dir exists and that we have a canonical path to the dir. Normally
        // we would want to avoid such an action at this layer, however to ensure that other layers
        // have a reliable FS path to work with, we make an exception here.
        let final_dist = opts
            .dist
            .unwrap_or_else(|| target_parent.join(super::DIST_DIR));
        if !final_dist.exists() {
            std::fs::create_dir(&final_dist).with_context(|| {
                format!("error creating final dist directory {:?}", &final_dist)
            })?;
        }
        let final_dist = final_dist
            .canonicalize()
            .context("error taking canonical path to dist dir")?;
        let staging_dist = final_dist.join(super::STAGE_DIR);

        // Highlander-rule: There can be only one (prohibits contradicting arguments):
        ensure!(
            !(opts.all_features && (opts.no_default_features || opts.features.is_some())),
            "Cannot combine --all-features with --no-default-features and/or --features"
        );

        let cargo_features = if opts.all_features {
            Features::All
        } else {
            Features::Custom {
                features: opts.features,
                no_default_features: opts.no_default_features,
            }
        };

        Ok(Self {
            target,
            target_parent,
            release: opts.release,
            public_url: opts.public_url.unwrap_or_else(|| "/".into()),
            filehash: opts.filehash.unwrap_or(true),
            staging_dist,
            final_dist,
            cargo_features,
            tools,
            hooks,
            inject_autoloader,
            inject_scripts: opts.inject_scripts.unwrap_or(true),
            pattern_script: opts.pattern_script,
            pattern_preload: opts.pattern_preload,
            pattern_params: opts.pattern_params,
            offline: opts.offline,
            frozen: opts.frozen,
            locked: opts.locked,
        })
    }

    /// Construct a new instance for testing.
    #[cfg(test)]
    pub async fn new_test(tmpdir: &std::path::Path) -> Result<Self> {
        let target = tmpdir.join("index.html");
        let target_parent = tmpdir.to_path_buf();
        let final_dist = tmpdir.join("dist");
        let staging_dist = final_dist.join(".stage");
        tokio::fs::create_dir_all(&staging_dist)
            .await
            .context("error creating dist & staging dir for test")?;
        Ok(Self {
            target,
            target_parent,
            release: false,
            public_url: "/".into(),
            filehash: true,
            final_dist,
            staging_dist,
            cargo_features: Features::All,
            tools: ConfigOptsTools {
                sass: None,
                wasm_bindgen: None,
                wasm_opt: None,
                tailwindcss: None,
            },
            hooks: Vec::new(),
            inject_autoloader: true,
            inject_scripts: true,
            pattern_script: None,
            pattern_preload: None,
            pattern_params: None,
            offline: false,
            frozen: false,
            locked: false,
        })
    }
}

/// Runtime config for the watch system.
#[derive(Clone, Debug)]
pub struct RtcWatch {
    /// Runtime config for the build system.
    pub build: Arc<RtcBuild>,
    /// Paths to watch, defaults to the build target parent directory.
    pub paths: Vec<PathBuf>,
    /// Paths to ignore.
    pub ignored_paths: Vec<PathBuf>,
    /// Polling mode for detecting changes if set to `Some(_)`.
    pub poll: Option<Duration>,
    /// Allow enabling a cooldown
    pub enable_cooldown: bool,
    /// No error reporting.
    pub no_error_reporting: bool,
}

impl RtcWatch {
    pub(super) fn new(
        build_opts: ConfigOptsBuild,
        opts: ConfigOptsWatch,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        inject_autoloader: bool,
        no_error_reporting: bool,
    ) -> Result<Self> {
        let build = Arc::new(RtcBuild::new(build_opts, tools, hooks, inject_autoloader)?);

        tracing::debug!("Disable error reporting: {no_error_reporting}");

        // Take the canonical path of each of the specified watch targets.
        let mut paths = vec![];
        for path in opts.watch.unwrap_or_default() {
            let canon_path = path
                .canonicalize()
                .map_err(|_| anyhow!("invalid watch path provided: {:?}", path))?;
            paths.push(canon_path);
        }
        // If no watch paths were provided, then we default to the target HTML's parent dir.
        if paths.is_empty() {
            paths.push(build.target_parent.clone());
        }

        // Take the canonical path of each of the specified ignore targets.
        let mut ignored_paths = match opts.ignore {
            None => vec![],
            Some(paths) => {
                paths
                    .into_iter()
                    .try_fold(vec![], |mut acc, path| -> Result<Vec<PathBuf>> {
                        let canon_path = path
                            .canonicalize()
                            .map_err(|_| anyhow!("invalid ignore path provided: {:?}", path))?;
                        acc.push(canon_path);
                        Ok(acc)
                    })?
            }
        };
        // Ensure the final dist dir is always ignored.
        ignored_paths.push(build.final_dist.clone());

        Ok(Self {
            build,
            paths,
            ignored_paths,
            poll: opts.poll.then(|| {
                opts.poll_interval
                    .map(|d| d.0)
                    .unwrap_or_else(|| Duration::from_secs(5))
            }),
            enable_cooldown: opts.enable_cooldown,
            no_error_reporting,
        })
    }
}

/// Runtime config for the serve system.
#[derive(Clone, Debug)]
pub struct RtcServe {
    /// Runtime config for the watch system.
    pub watch: Arc<RtcWatch>,
    /// The IP address to serve on.
    pub address: IpAddr,
    /// The port to serve on.
    pub port: u16,
    /// Open a browser tab once the initial build is complete.
    pub open: bool,
    /// A URL to which requests will be proxied.
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend.
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets.
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure connections.
    pub proxy_insecure: bool,
    /// Any proxies configured to run along with the server.
    pub proxies: Option<Vec<ConfigOptsProxy>>,
    /// Whether to disable auto-reload of the web page when a build completes.
    pub no_autoreload: bool,
    /// Additional headers to include in responses.
    pub headers: HashMap<String, String>,
    /// Protocol used for autoreload WebSockets connection.
    pub ws_protocol: Option<WsProtocol>,
    /// The tls config containing the certificate and private key. TLS is activated if both are set.
    pub tls: Option<RustlsConfig>,
}

impl RtcServe {
    pub(super) async fn new(
        build_opts: ConfigOptsBuild,
        watch_opts: ConfigOptsWatch,
        opts: ConfigOptsServe,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        proxies: Option<Vec<ConfigOptsProxy>>,
    ) -> Result<Self> {
        let watch = Arc::new(RtcWatch::new(
            build_opts,
            watch_opts,
            tools,
            hooks,
            !opts.no_autoreload,
            opts.no_error_reporting,
        )?);
        let tls = tls_config(
            absolute_path_if_some(opts.tls_key_path, "tls_key_path")?,
            absolute_path_if_some(opts.tls_cert_path, "tls_cert_path")?,
        )
        .await?;
        Ok(Self {
            watch,
            address: opts.address.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port: opts.port.unwrap_or(8080),
            open: opts.open,
            proxy_backend: opts.proxy_backend,
            proxy_rewrite: opts.proxy_rewrite,
            proxy_insecure: opts.proxy_insecure,
            proxy_ws: opts.proxy_ws,
            proxies,
            no_autoreload: opts.no_autoreload,
            headers: opts.headers,
            ws_protocol: opts.ws_protocol,
            tls,
        })
    }
}

async fn tls_config(
    tls_key_path: Option<PathBuf>,
    tls_cert_path: Option<PathBuf>,
) -> Result<Option<RustlsConfig>, anyhow::Error> {
    match (tls_key_path, tls_cert_path) {
        (Some(tls_key_path), Some(tls_cert_path)) => {
            tracing::info!("🔐 Private key {}", tls_key_path.display(),);
            tracing::info!("🔒 Public key {}", tls_cert_path.display());
            let tls_config = RustlsConfig::from_pem_file(tls_cert_path, tls_key_path)
                .await
                .with_context(|| "loading TLS cert/key failed")?;
            Ok(Some(tls_config))
        }
        (None, Some(_)) => Err(anyhow!("TLS cert path provided without key path")),
        (Some(_), None) => Err(anyhow!("TLS key path provided without cert path")),
        (None, None) => Ok(None),
    }
}

fn absolute_path_if_some(
    maybe_path: Option<PathBuf>,
    file_description: &str,
) -> Result<Option<PathBuf>, anyhow::Error> {
    match maybe_path {
        Some(path) => Ok(Some(absolute_path(path, file_description)?)),
        None => Ok(None),
    }
}

fn absolute_path(path: PathBuf, file_description: &str) -> Result<PathBuf, anyhow::Error> {
    path.canonicalize().with_context(|| {
        format!(
            "error getting canonical path to {} file {:?}",
            file_description, &path
        )
    })
}

/// Runtime config for the clean system.
#[derive(Clone, Debug)]
pub struct RtcClean {
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
}

impl RtcClean {
    pub(super) fn new(opts: ConfigOptsClean) -> Self {
        Self {
            dist: opts.dist.unwrap_or_else(|| super::DIST_DIR.into()),
            cargo: opts.cargo,
        }
    }
}
