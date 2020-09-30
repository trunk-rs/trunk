use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use http_types::Url;

use crate::config::{CargoMetadata, ConfigOptsBuild, ConfigOptsClean, ConfigOptsProxy, ConfigOptsServe, ConfigOptsWatch};

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// The metadata of the associated cargo project being processed.
    pub manifest: CargoMetadata,
    /// The public URL from which assets are to be served.
    pub public_url: String,
}

impl RtcBuild {
    /// Construct a new instance.
    pub(super) fn new(manifest: CargoMetadata, opts: ConfigOptsBuild) -> Result<Self> {
        let target = opts.target.clone().unwrap_or_else(|| "index.html".into());
        Ok(Self {
            target: target
                .canonicalize()
                .with_context(|| format!("error getting canonical path to source HTML file {:?}", &target))?,
            release: opts.release,
            // Use a config defined value.
            dist: opts.dist.unwrap_or_else(||
                // Else fallback to the parent dir of the cargo target dir.
                manifest.metadata.target_directory
                    .parent().map(|path| path.join("dist"))
                    // Else fallback to the "dist" dir of the CWD (this will practically never be hit).
                    .unwrap_or_else(|| "dist".into())),
            manifest,
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
    pub(super) fn new(manifest: CargoMetadata, build_opts: ConfigOptsBuild, opts: ConfigOptsWatch) -> Result<Self> {
        let build = Arc::new(RtcBuild::new(manifest, build_opts)?);
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
        manifest: CargoMetadata, build_opts: ConfigOptsBuild, watch_opts: ConfigOptsWatch, opts: ConfigOptsServe,
        proxies: Option<Vec<ConfigOptsProxy>>,
    ) -> Result<Self> {
        let watch = Arc::new(RtcWatch::new(manifest, build_opts, watch_opts)?);
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
