use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventHandler, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_stream::wrappers::BroadcastStream;

use crate::build::BuildSystem;
use crate::config::RtcWatch;

/// Blacklisted path segments which are ignored by the watcher by default.
const BLACKLIST: [&str; 1] = [".git"];

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system.
    build: Arc<Mutex<BuildSystem>>,
    /// The current vector of paths to be ignored.
    ignored_paths: Arc<RwLock<Vec<PathBuf>>>,
    /// A channel of new paths to ignore from the build system.
    build_rx: mpsc::Receiver<PathBuf>,
    /// The watch system used for watching the filesystem.
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    /// The application shutdown channel.
    shutdown: BroadcastStream<()>,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(
        cfg: Arc<RtcWatch>,
        shutdown: broadcast::Sender<()>,
        build_done_tx: Option<broadcast::Sender<()>>,
    ) -> Result<Self> {
        let runtime = tokio::runtime::Handle::current();

        // Create a channel for being able to listen for new paths to ignore while running.
        let (build_tx, build_rx) = mpsc::channel(1);

        // Build dependencies.
        let build = Arc::new(Mutex::new(
            BuildSystem::new(cfg.build.clone(), Some(build_tx)).await?,
        ));

        let ignored_paths = Arc::new(RwLock::new(cfg.ignored_paths.clone()));

        let mut inner = ChangeHandler {
            ignored_paths: ignored_paths.clone(),
            build_done_tx,
            build: build.clone(),
            runtime,
        };

        // Build the watcher.
        let _debouncer = build_watcher(
            move |events: DebounceEventResult| match events {
                Ok(events) => {
                    inner.handle_watch_events(events);
                }
                Err(errs) => {
                    for (n, err) in errs.into_iter().enumerate() {
                        tracing::info!("Error while watching - {n:03}: {err}");
                    }
                }
            },
            cfg.paths.clone(),
        )?;

        Ok(Self {
            build,
            _debouncer,
            ignored_paths,
            build_rx,
            shutdown: BroadcastStream::new(shutdown.subscribe()),
        })
    }

    /// Run a build.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn build(&mut self) -> Result<()> {
        self.build.lock().await.build().await
    }

    /// Run the watch system, responding to events and triggering builds.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(ign) = self.build_rx.recv() => self.update_ignore_list(ign),
                _ = self.shutdown.next() => break, // Any event, even a drop, will trigger shutdown.
            }
        }

        tracing::debug!("watcher system has shut down");
    }

    fn update_ignore_list(&self, arg_path: PathBuf) {
        let path = match arg_path.canonicalize() {
            Ok(canon_path) => canon_path,
            Err(_) => arg_path,
        };

        let mut ignored_paths = self.ignored_paths.write().expect("Failed to acquire lock");
        if !ignored_paths.contains(&path) {
            ignored_paths.push(path);
        }
    }
}

/// Build a FS watcher and debouncer, when it is dropped, it will stop watching for events.
fn build_watcher<H: DebounceEventHandler>(
    event_handler: H,
    paths: Vec<PathBuf>,
) -> Result<Debouncer<RecommendedWatcher, FileIdMap>> {
    let mut debouncer = new_debouncer(Duration::from_secs(1), None, event_handler)
        .context("failed to build file system watcher")?;

    // Create a recursive watcher on each of the given paths.
    // NOTE WELL: it is expected that all given paths are canonical. The Trunk config
    // system currently ensures that this is true for all data coming from the
    // RtcBuild/RtcWatch/RtcServe/&c runtime config objects.
    for path in paths {
        debouncer
            .watcher()
            .watch(&path, RecursiveMode::Recursive)
            .context(format!(
                "failed to watch {:?} for file system changes",
                path
            ))?;
    }

    Ok(debouncer)
}

/// The handler for filesystem changes.
struct ChangeHandler {
    /// Runtime handle, for spawning futures.
    runtime: Handle,
    /// The build system.
    build: Arc<Mutex<BuildSystem>>,
    /// The current vector of paths to be ignored.
    ignored_paths: Arc<RwLock<Vec<PathBuf>>>,
    /// Channel that is sent on whenever a build completes.
    build_done_tx: Option<broadcast::Sender<()>>,
}

impl ChangeHandler {
    /// Test if an event is relevant to our configuration.
    fn is_relevant(&self, ev_path: &Path) -> bool {
        let ev_path = match std::fs::canonicalize(ev_path) {
            Ok(ev_path) => ev_path,
            // Ignore errors here, as this would only take place for a resource which has
            // been removed, which will happen for each of our dist/.stage entries.
            Err(_) => return false,
        };

        // Check ignored paths.
        let ignored_paths = self.ignored_paths.read().expect("Failed to acquire lock");
        if ev_path.ancestors().any(|path| {
            ignored_paths
                .iter()
                .any(|ignored_path| ignored_path == path)
        }) {
            return false; // Don't emit a notification if path is ignored.
        }

        // Check blacklisted paths.
        if ev_path
            .components()
            .filter_map(|segment| segment.as_os_str().to_str())
            .any(|segment| BLACKLIST.contains(&segment))
        {
            return false; // Don't emit a notification as path is on the blacklist.
        }

        tracing::info!("change detected in {:?}", ev_path);

        true
    }

    /// Handle an array of [`DebouncedEvent`]s. If any of them is relevant, we run a new build,
    /// and wait for it finish before returning, so that the debouncer knows we are ready for the
    /// next step.
    #[tracing::instrument(level = "trace", skip(self), fields(events = events.len()))]
    fn handle_watch_events(&mut self, events: Vec<DebouncedEvent>) {
        // check if we have any relevant change event
        let mut none = true;
        for path in events.iter().flat_map(|event| &event.paths) {
            if self.is_relevant(path) {
                none = false;
                break;
            }
        }

        if none {
            // none of the events was relevant
            return;
        }

        let (once_tx, once_rx) = tokio::sync::oneshot::channel();
        let build = self.build.clone();
        self.runtime.spawn(async move {
            let mut build = build.lock().await;
            let _ = once_tx.send(build.build().await);
        });

        // wait until the spawned build is ready, and retrieve its result
        let _ret = once_rx.blocking_recv();

        // TODO/NOTE: in the future, we will want to be able to pass along error info and other
        // diagnostics info over the socket for use in an error overlay or console logging.
        if let Some(tx) = self.build_done_tx.as_mut() {
            let _ = tx.send(());
        }
    }
}
