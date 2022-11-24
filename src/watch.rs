use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;

use crate::build::BuildSystem;
use crate::config::RtcWatch;

/// Blacklisted path segments which are ignored by the watcher by default.
const BLACKLIST: [&str; 1] = [".git"];

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system.
    build: BuildSystem,
    /// The current vector of paths to be ignored.
    ignored_paths: Vec<PathBuf>,
    /// A channel of FS watch events.
    watch_rx: mpsc::Receiver<DebouncedEvent>,
    /// A channel of new paths to ignore from the build system.
    build_rx: mpsc::Receiver<PathBuf>,
    /// The watch system used for watching the filesystem.
    _watcher: RecommendedWatcher,
    /// The application shutdown channel.
    shutdown: BroadcastStream<()>,
    /// Channel that is sent on whenever a build completes.
    build_done_tx: Option<broadcast::Sender<()>>,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(
        cfg: Arc<RtcWatch>,
        shutdown: broadcast::Sender<()>,
        build_done_tx: Option<broadcast::Sender<()>>,
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
            build_done_tx,
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
        // Create a channel that listens for relevant changes.
        let mut last_update: bool = false;
        let (trigger_tx, trigger_rx) = tokio::sync::watch::channel(last_update);

        // Todo - somehow make this async
        // Trigger builds for relevant changes only.
        std::thread::spawn(move || {
            loop {
                tokio::select! {
                    Some(event) = self.watch_rx.recv() => {
                         if let Some(path) = self.filter_watch_event(event).await {
                             if trigger_tx.send(true).is_err() {
                                break;
                            }
                        }
                    },
                    _ = self.shutdown.next() => break, // Any event, even a drop, will trigger shutdown.
                }
            }
        });

        loop {
            tokio::select! {
                Some(ign) = self.build_rx.recv() => self.update_ignore_list(ign),
                _ = trigger_rx.changed() => self.trigger_build().await,
                _ = self.shutdown.next() => break, // Any event, even a drop, will trigger shutdown.
            }
        }

        tracing::debug!("watcher system has shut down");
    }

    #[tracing::instrument(level = "trace", skip(self, event))]
    async fn filter_watch_event(&mut self, event: DebouncedEvent) -> Option<PathBuf> {
        let ev_path = match event {
            DebouncedEvent::Create(path)
            | DebouncedEvent::Write(path)
            | DebouncedEvent::Remove(path)
            | DebouncedEvent::Rename(_, path) => path,
            DebouncedEvent::NoticeWrite(_)
            | DebouncedEvent::NoticeRemove(_)
            | DebouncedEvent::Chmod(_)
            | DebouncedEvent::Rescan
            | DebouncedEvent::Error(..) => return None,
        };

        let ev_path = match tokio::fs::canonicalize(&ev_path).await {
            Ok(ev_path) => ev_path,
            // Ignore errors here, as this would only take place for a resource which has
            // been removed, which will happen for each of our dist/.stage entries.
            Err(_) => return None,
        };

        // Check ignored paths.
        if ev_path.ancestors().any(|path| {
            self.ignored_paths
                .iter()
                .any(|ignored_path| ignored_path == path)
        }) {
            return None; // Don't emit a notification if path is ignored.
        }

        // Check blacklisted paths.
        if ev_path
            .components()
            .filter_map(|segment| segment.as_os_str().to_str())
            .any(|segment| BLACKLIST.contains(&segment))
        {
            return None; // Don't emit a notification as path is on the blacklist.
        }

        tracing::debug!("change detected in {:?}", ev_path);
        Some(ev_path)
    }

    async fn trigger_build(&mut self) {
        let _res = self.build.build().await;

        // TODO/NOTE: in the future, we will want to be able to pass along error info and other
        // diagnostics info over the socket for use in an error overlay or console logging.
        if let Some(tx) = self.build_done_tx.as_mut() {
            let _ = tx.send(());
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
fn build_watcher(
    watch_tx: mpsc::Sender<DebouncedEvent>,
    paths: Vec<PathBuf>,
) -> Result<RecommendedWatcher> {
    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        while let Ok(event) = watcher_rx.recv() {
            if watch_tx.send(event).is_err() {
                break;
            }
        }
    });

    let mut watcher = notify::watcher(watcher_tx, Duration::from_secs(1))
        .context("failed to build file system watcher")?;

    // Create a recursive watcher on each of the given paths.
    // NOTE WELL: it is expected that all given paths are canonical. The Trunk config
    // system currently ensures that this is true for all data coming from the
    // RtcBuild/RtcWatch/RtcServe/&c runtime config objects.
    for path in paths {
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .context(format!(
                "failed to watch {:?} for file system changes",
                path
            ))?;
    }

    Ok(watcher)
}
