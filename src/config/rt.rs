use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use axum::http::Uri;

use crate::config::{ConfigOptsBuild, ConfigOptsClean, ConfigOptsHook, ConfigOptsProxy, ConfigOptsServe, ConfigOptsTools, ConfigOptsWatch};

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// The parent directory of the target index HTML file.
    pub target_parent: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// The public URL from which assets are to be served.
    pub public_url: String,
    /// The directory where final build artifacts are placed after a successful build.
    pub final_dist: PathBuf,
    /// The directory used to stage build artifacts during an active build.
    pub staging_dist: PathBuf,
    /// Configuration for automatic application download.
    pub tools: ConfigOptsTools,
    /// Build process hooks.
    pub hooks: Vec<ConfigOptsHook>,
    /// A bool indicating if the output HTML should have the WebSocket autoloader injected.
    ///
    /// This value is configured via the server config only. If the server is not being used, then
    /// the autoloader will not be injected.
    pub inject_autoloader: bool,
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
    pub(super) fn new(opts: ConfigOptsBuild, tools: ConfigOptsTools, hooks: Vec<ConfigOptsHook>, inject_autoloader: bool) -> Result<Self> {
        // Get the canonical path to the target HTML file.
        let pre_target = opts.target.clone().unwrap_or_else(|| "index.html".into());
        let target = pre_target
            .canonicalize()
            .with_context(|| format!("error getting canonical path to source HTML file {:?}", &pre_target))?;

        // Get the target HTML's parent dir, falling back to OS specific root, as that is the only
        // time where no parent could be determined.
        let target_parent = target
            .parent()
            .map(|path| path.to_owned())
            .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));

        // Ensure the final dist dir exists and that we have a canonical path to the dir. Normally
        // we would want to avoid such an action at this layer, however to ensure that other layers
        // have a reliable FS path to work with, we make an exception here.
        let final_dist = opts.dist.unwrap_or_else(|| target_parent.join(super::DIST_DIR));
        if !final_dist.exists() {
            std::fs::create_dir(&final_dist).with_context(|| format!("error creating final dist directory {:?}", &final_dist))?;
        }
        let final_dist = final_dist
            .canonicalize()
            .context("error taking canonical path to dist dir")?;
        let staging_dist = final_dist.join(super::STAGE_DIR);

        Ok(Self {
            target,
            target_parent,
            release: opts.release,
            staging_dist,
            final_dist,
            public_url: opts.public_url.unwrap_or_else(|| "/".into()),
            tools,
            hooks,
            inject_autoloader,
            pattern_script: opts.pattern_script,
            pattern_preload: opts.pattern_preload,
            pattern_params: opts.pattern_params,
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
}

impl RtcWatch {
    pub(super) fn new(
        build_opts: ConfigOptsBuild, opts: ConfigOptsWatch, tools: ConfigOptsTools, hooks: Vec<ConfigOptsHook>, inject_autoloader: bool,
    ) -> Result<Self> {
        let build = Arc::new(RtcBuild::new(build_opts, tools, hooks, inject_autoloader)?);

        // Take the canonical path of each of the specified watch targets.
        let mut paths = vec![];
        for path in opts.watch.unwrap_or_default() {
            let canon_path = path
                .canonicalize()
                .map_err(|_| anyhow!("invalid watch path provided: {:?}", path))?;
            paths.push(canon_path);
        }
        // If no watch paths were provied, then we default to the target HTML's parent dir.
        if paths.is_empty() {
            paths.push(build.target_parent.clone());
        }

        // Take the canonical path of each of the specified ignore targets.
        let mut ignored_paths = match opts.ignore {
            None => vec![],
            Some(paths) => paths
                .into_iter()
                .try_fold(vec![], |mut acc, path| -> Result<Vec<PathBuf>> {
                    let canon_path = path
                        .canonicalize()
                        .map_err(|_| anyhow!("invalid ignore path provided: {:?}", path))?;
                    acc.push(canon_path);
                    Ok(acc)
                })?,
        };
        // Ensure the final dist dir is always ignored.
        ignored_paths.push(build.final_dist.clone());

        Ok(Self { build, paths, ignored_paths })
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
    /// Any proxies configured to run along with the server.
    pub proxies: Option<Vec<ConfigOptsProxy>>,
    /// Whether to disable auto-reload of the web page when a build completes.
    pub no_autoreload: bool,
}

impl RtcServe {
    pub(super) fn new(
        build_opts: ConfigOptsBuild, watch_opts: ConfigOptsWatch, opts: ConfigOptsServe, tools: ConfigOptsTools, hooks: Vec<ConfigOptsHook>,
        proxies: Option<Vec<ConfigOptsProxy>>,
    ) -> Result<Self> {
        let watch = Arc::new(RtcWatch::new(build_opts, watch_opts, tools, hooks, !opts.no_autoreload)?);
        Ok(Self {
            watch,
            address: opts.address.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port: opts.port.unwrap_or(8080),
            open: opts.open,
            proxy_backend: opts.proxy_backend,
            proxy_rewrite: opts.proxy_rewrite,
            proxy_ws: opts.proxy_ws,
            proxies,
            no_autoreload: opts.no_autoreload,
        })
    }
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
