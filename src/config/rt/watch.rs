use crate::config::{
    rt::{BuildOptions, RtcBuild, RtcBuilder},
    Configuration, Watch,
};
use anyhow::anyhow;
use std::{ops::Deref, path::PathBuf, sync::Arc, time::Duration};

/// Tracks the patterns added to a `globset::GlobSet`
/// so the matcher can be updated.
#[derive(Clone, Debug)]
pub struct GlobMatcher {
    patterns: Vec<globset::Glob>,
    matcher: globset::GlobSet,
}

impl GlobMatcher {
    /// Create a new `GlobMatcher`.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            matcher: globset::GlobSet::empty(),
        }
    }

    /// Add a new pattern to the matcher.
    ///
    /// # Note
    /// This is somewhat expensive because it needs to recreate the matcher.
    pub fn add(&mut self, pattern: globset::Glob) -> Result<(), globset::Error> {
        let mut matcher = globset::GlobSet::builder();
        for pattern in self.patterns.iter().cloned() {
            matcher.add(pattern);
        }
        matcher.add(pattern.clone());
        let matcher = matcher.build()?;

        self.patterns.push(pattern);
        self.matcher = matcher;
        Ok(())
    }

    /// Returns true if any glob the set matches the path given.
    pub fn is_match(&self, path: impl AsRef<std::path::Path>) -> bool {
        self.matcher.is_match(path.as_ref())
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
    pub ignored_paths: GlobMatcher,
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

        let mut ignored_paths = GlobMatcher::new();

        // Ensure the final dist dir is always ignored.
        let Some(final_dist) = build.final_dist.to_str() else {
            return Err(anyhow!("could not convert final distribution path to glob"));
        };
        let final_dist = globset::Glob::new(final_dist).map_err(|err| anyhow!(err))?;
        ignored_paths.add(final_dist).map_err(|err| anyhow!(err))?;

        let final_dist_recursive = build.final_dist.join("**");
        let Some(final_dist_recursive) = final_dist_recursive.to_str() else {
            return Err(anyhow!("could not convert final distribution path to glob"));
        };
        let final_dist_recursive =
            globset::Glob::new(final_dist_recursive).map_err(|err| anyhow!(err))?;
        ignored_paths
            .add(final_dist_recursive)
            .map_err(|err| anyhow!(err))?;

        let working_dir = build
            .working_directory
            .canonicalize()
            .map_err(|_| anyhow!("error taking the canonical path to the working directory"))?;
        for path in ignore {
            let path = working_dir.join(path);
            let Some(glob) = path.to_str() else {
                return Err(anyhow!("could not convert {:?} to str", path));
            };
            let glob = globset::Glob::new(glob).map_err(|err| anyhow!(err))?;
            ignored_paths.add(glob).map_err(|err| anyhow!(err))?;

            // Add recursive path for directories or file system objects
            // that do not exist on disk. This maintains the previous behavior
            // that paths are automatically recursive.
            if !path.is_file() {
                let path = path.join("**");
                let Some(glob) = path.to_str() else {
                    return Err(anyhow!("could not convert {:?} to str", path));
                };
                let glob = globset::Glob::new(glob).map_err(|err| anyhow!(err))?;
                ignored_paths.add(glob).map_err(|err| anyhow!(err))?;
            }
        }

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
