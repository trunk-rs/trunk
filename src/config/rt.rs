use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use http_types::Url;

use crate::config::{ConfigOptsBuild, ConfigOptsClean, ConfigOptsProxy, ConfigOptsServe, ConfigOptsWatch};

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// The public URL from which assets are to be served.
    pub public_url: String,
}

impl RtcBuild {
    /// Construct a new instance.
    pub(super) fn new(opts: ConfigOptsBuild) -> Result<Self> {
        let pre_target = opts.target.clone().unwrap_or_else(|| "index.html".into());
        let target = pre_target
            .canonicalize()
            .with_context(|| format!("error getting canonical path to source HTML file {:?}", &pre_target))?;
        let target_parent_dir = target
            .parent()
            .map(|path| path.to_owned())
            .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));
        Ok(Self {
            target,
            release: opts.release,
            dist: opts.dist.unwrap_or_else(|| target_parent_dir.join("dist")),
            public_url: opts.public_url.unwrap_or_else(|| "/".into()),
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
    pub(super) fn new(build_opts: ConfigOptsBuild, opts: ConfigOptsWatch) -> Result<Self> {
        let build = Arc::new(RtcBuild::new(build_opts)?);

        let paths = {
            let mut paths = opts.watch.unwrap_or_default();

            if paths.is_empty() {
                paths.push(
                    build
                        .target
                        .parent()
                        .ok_or_else(|| anyhow!("couldn't get parent of {:?}", build.target))?
                        .to_path_buf(),
                )
            }

            paths
        };

        Ok(Self {
            build,
            paths,
            ignored_paths: opts.ignore.unwrap_or_default(),
        })
    }
}

/// Runtime config for the serve system.
#[derive(Clone, Debug)]
pub struct RtcServe {
    /// Runtime config for the watch system.
    pub watch: Arc<RtcWatch>,
    /// The port to serve on.
    pub port: u16,
    /// Open a browser tab once the initial build is complete.
    pub open: bool,
    /// A URL to which requests will be proxied.
    pub proxy_backend: Option<Url>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend.
    pub proxy_rewrite: Option<String>,
    /// Any proxies configured to run along with the server.
    pub proxies: Option<Vec<ConfigOptsProxy>>,
}

impl RtcServe {
    pub(super) fn new(
        build_opts: ConfigOptsBuild, watch_opts: ConfigOptsWatch, opts: ConfigOptsServe, proxies: Option<Vec<ConfigOptsProxy>>,
    ) -> Result<Self> {
        let watch = Arc::new(RtcWatch::new(build_opts, watch_opts)?);
        Ok(Self {
            watch,
            port: opts.port.unwrap_or(8080),
            open: opts.open,
            proxy_backend: opts.proxy_backend,
            proxy_rewrite: opts.proxy_rewrite,
            proxies,
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
    pub(super) fn new(opts: ConfigOptsClean) -> Result<Self> {
        Ok(Self {
            dist: opts.dist.unwrap_or_else(|| "dist".into()),
            cargo: opts.cargo,
        })
    }
}
