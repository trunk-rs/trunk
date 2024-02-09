use crate::config::{ConfigOptsBuild, ConfigOptsHook, ConfigOptsTools, ConfigOptsWatch};
use anyhow::anyhow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Runtime config for the watch system.
#[derive(Clone, Debug)]
pub struct RtcWatch {
    /// Runtime config for the build system.
    pub build: Arc<super::RtcBuild>,
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
    pub(crate) fn new(
        build_opts: ConfigOptsBuild,
        opts: ConfigOptsWatch,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        inject_autoloader: bool,
        no_error_reporting: bool,
    ) -> anyhow::Result<Self> {
        let build = Arc::new(super::RtcBuild::new(
            build_opts,
            tools,
            hooks,
            inject_autoloader,
        )?);

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
            Some(paths) => paths.into_iter().try_fold(
                vec![],
                |mut acc, path| -> anyhow::Result<Vec<PathBuf>> {
                    let canon_path = path
                        .canonicalize()
                        .map_err(|_| anyhow!("invalid ignore path provided: {:?}", path))?;
                    acc.push(canon_path);
                    Ok(acc)
                },
            )?,
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
