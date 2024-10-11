use crate::config::{
    rt::{BuildOptions, RtcBuild, RtcBuilder},
    Configuration, Watch,
};
use anyhow::anyhow;
use std::{ops::Deref, path::PathBuf, sync::Arc, time::Duration};

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
    /// Clear the screen before each run
    pub clear_screen: bool,
    /// No error reporting.
    pub no_error_reporting: bool,
}

impl Deref for RtcWatch {
    type Target = RtcBuild;

    fn deref(&self) -> &Self::Target {
        &self.build
    }
}

#[derive(Clone, Debug)]
pub struct WatchOptions {
    pub build: BuildOptions,
    /// Polling mode for detecting changes if set to `Some(_)`.
    pub poll: Option<Duration>,
    /// Allow enabling a cooldown
    pub enable_cooldown: bool,
    /// Clear the screen before each run
    pub clear_screen: bool,
    /// No error reporting.
    pub no_error_reporting: bool,
}

impl RtcWatch {
    /// Construct a new instance.
    pub(crate) fn new(config: Configuration, opts: WatchOptions) -> anyhow::Result<Self> {
        let WatchOptions {
            build: build_opts,
            poll,
            enable_cooldown,
            clear_screen,
            no_error_reporting,
        } = opts;

        let Watch { watch, ignore } = config.watch.clone();

        let build = RtcBuild::new(config, build_opts)?;

        tracing::debug!("Disable error reporting: {no_error_reporting}");

        // Take the canonical path of each of the specified watch targets.
        let mut paths = vec![];
        for path in watch {
            let path = build.working_directory.join(path);
            let canon_path = path.canonicalize().map_err(|_| {
                anyhow!(
                    "error taking the canonical path to the watch path: {:?}",
                    path
                )
            })?;
            paths.push(canon_path);
        }

        // If no watch paths were provided, then we default to the target HTML's parent dir.
        if paths.is_empty() {
            paths.push(build.target_parent.clone());
        }

        // Take the canonical path of each of the specified ignore targets.
        let mut ignored_paths = ignore
            .into_iter()
            .map(|path| {
                let path = build.working_directory.join(path);
                path.canonicalize().map_err(|_| {
                    anyhow!(
                        "error taking the canonical path to the watch ignore path: {:?}",
                        path
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Ensure the final dist dir is always ignored.
        ignored_paths.push(build.final_dist.clone());

        Ok(Self {
            build: Arc::new(build),
            paths,
            ignored_paths,
            poll,
            enable_cooldown,
            clear_screen,
            no_error_reporting,
        })
    }
}

impl RtcBuilder for RtcWatch {
    type Options = WatchOptions;

    async fn build(configuration: Configuration, options: Self::Options) -> anyhow::Result<Self> {
        Self::new(configuration, options)
    }
}
