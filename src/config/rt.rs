use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use http_types::Url;

use crate::config::{ConfigOptsBuild, ConfigOptsClean, ConfigOptsProxy, ConfigOptsServe, ConfigOptsWatch};

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// The directory to which plugins should output final assets.
    pub staging_dist: PathBuf,
    /// The final resting place where assets will be moved to by Trunk.
    pub final_dist: PathBuf,
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
        let final_dist = opts.dist.unwrap_or_else(|| target_parent_dir.join("dist"));
        let staging_dist = final_dist.join(".current");
        Ok(Self {
            target,
            release: opts.release,
            staging_dist,
            final_dist,
            public_url: opts.public_url.unwrap_or_else(|| "/".into()),
        })
    }
}

/// Runtime config for the watch system.
#[derive(Clone, Debug)]
pub struct RtcWatch {
    /// Runtime config for the build system.
    pub build: Arc<RtcBuild>,
    /// Additional paths to ignore.
    pub ignore: Vec<PathBuf>,
}

impl RtcWatch {
    pub(super) fn new(build_opts: ConfigOptsBuild, opts: ConfigOptsWatch) -> Result<Self> {
        let build = Arc::new(RtcBuild::new(build_opts)?);
        Ok(Self {
            build,
            ignore: opts.ignore.unwrap_or_default(),
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
    /// The final resting place where assets will be moved to by Trunk.
    pub final_dist: PathBuf,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
}

impl RtcClean {
    pub(super) fn new(opts: ConfigOptsClean) -> Result<Self> {
        Ok(Self {
            final_dist: opts.dist.unwrap_or_else(|| "dist".into()),
            cargo: opts.cargo,
        })
    }
}
