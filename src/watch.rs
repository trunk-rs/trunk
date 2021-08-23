use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::prelude::*;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;

use crate::build::BuildSystem;
use crate::config::RtcWatch;

/// Blacklisted path segments which are ignored by the watcher by default.
const BLACKLIST: [&str; 1] = [".git"];

/// A message sent by the new build status broadcaster.
#[derive(Clone, PartialEq, Eq)]
pub enum NewBuildStatusMsg {
    /// A new build has started.
    BuildStarted,
    /// A build has completed without errors.
    BuildSucceeded,
    /// A build has completed with errors.
    BuildFailed,
}

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system.
    build: BuildSystem,
    /// The current vector of paths to be ignored.
    ignored_paths: Vec<PathBuf>,
    /// A channel of FS watch events.
    watch_rx: mpsc::Receiver<Event>,
    /// A channel of new paths to ignore from the build system.
    build_rx: mpsc::Receiver<PathBuf>,
    /// The watch system used for watching the filesystem.
    _watcher: RecommendedWatcher,
    /// The application shutdown channel.
    shutdown: BroadcastStream<()>,
    /// Channel that is sent on whenever a the build status changes.
    new_build_status_chan: Option<broadcast::Sender<NewBuildStatusMsg>>,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(
        cfg: Arc<RtcWatch>, shutdown: broadcast::Sender<()>, new_build_status_chan: Option<broadcast::Sender<NewBuildStatusMsg>>,
    ) -> Result<Self> {
        // Create a channel for being able to listen for new paths to ignore while running.
        let (watch_tx, watch_rx) = mpsc::channel(1);
        let (build_tx, build_rx) = mpsc::channel(1);

        // Build the watcher.
        let _watcher = build_watcher(watch_tx, cfg.paths.clone())?;

        // Build dependencies.
        let build = BuildSystem::new(cfg.build.clone(), Some(build_tx)).await?;
        Ok(Self {
            build,
            ignored_paths: cfg.ignored_paths.clone(),
            watch_rx,
            build_rx,
            _watcher,
            shutdown: BroadcastStream::new(shutdown.subscribe()),
            new_build_status_chan,
        })
    }

    /// Run a build.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn build(&mut self) -> Result<()> {
        self.build.build().await
    }

    /// Run the watch system, responding to events and triggering builds.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(ign) = self.build_rx.recv() => self.update_ignore_list(ign),
                Some(ev) = self.watch_rx.recv() => self.handle_watch_event(ev).await,
                _ = self.shutdown.next() => break, // Any event, even a drop, will trigger shutdown.
            }
        }

        tracing::debug!("watcher system has shut down");
    }

    #[tracing::instrument(level = "trace", skip(self, event))]
    async fn handle_watch_event(&mut self, event: Event) {
        if matches!(&event.kind, EventKind::Access(_) | EventKind::Any | EventKind::Other) {
            return; // Nothing to do with these.
        }

        for ev_path in event.paths {
            let ev_path = match tokio::fs::canonicalize(&ev_path).await {
                Ok(ev_path) => ev_path,
                // Ignore errors here, as this would only take place for a resource which has
                // been removed, which will happen for each of our dist/.stage entries.
                Err(_) => continue,
            };

            // Check ignored paths.
            if ev_path
                .ancestors()
                .any(|path| self.ignored_paths.iter().any(|ignored_path| ignored_path == path))
            {
                continue; // Don't emit a notification if path is ignored.
            }

            // Check blacklisted paths.
            if ev_path
                .components()
                .filter_map(|segment| segment.as_os_str().to_str())
                .any(|segment| BLACKLIST.contains(&segment))
            {
                continue; // Don't emit a notification as path is on the blacklist.
            }

            tracing::debug!("change detected in {:?}", ev_path);
            if let Some(tx) = self.new_build_status_chan.as_mut() {
                let _ = tx.send(NewBuildStatusMsg::BuildStarted);
            }
            match self.build.build().await {
                Ok(_) => {
                    if let Some(tx) = self.new_build_status_chan.as_mut() {
                        let _ = tx.send(NewBuildStatusMsg::BuildSucceeded);
                    }
                }
                Err(_) => {
                    if let Some(tx) = self.new_build_status_chan.as_mut() {
                        let _ = tx.send(NewBuildStatusMsg::BuildFailed);
                    }
                }
            }

            return; // If one of the paths triggers a build, then we're done.
        }
    }

    fn update_ignore_list(&mut self, arg_path: PathBuf) {
        let path = match arg_path.canonicalize() {
            Ok(canon_path) => canon_path,
            Err(_) => arg_path,
        };

        if !self.ignored_paths.contains(&path) {
            self.ignored_paths.push(path);
        }
    }
}

/// Build a FS watcher, when the watcher is dropped, it will stop watching for events.
fn build_watcher(watch_tx: mpsc::Sender<Event>, paths: Vec<PathBuf>) -> Result<RecommendedWatcher> {
    let event_handler = move |event_res: notify::Result<Event>| match event_res {
        Ok(event) => {
            let _res = watch_tx.try_send(event);
        }
        Err(err) => {
            tracing::error!(error = ?err, "error from FS watcher");
        }
    };
    let mut watcher = recommended_watcher(event_handler).context("failed to build file system watcher")?;

    // Create a recursive watcher on each of the given paths.
    // NOTE WELL: it is expected that all given paths are canonical. The Trunk config
    // system currently ensures that this is true for all data coming from the
    // RtcBuild/RtcWatch/RtcServe/&c runtime config objects.
    for path in paths {
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .context(format!("failed to watch {:?} for file system changes", path))?;
    }

    Ok(watcher)
}
