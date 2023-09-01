use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;
use tokio_stream::wrappers::BroadcastStream;

use crate::build::BuildSystem;
use crate::config::{RtcWatch, WsProtocol};

/// The debouncer type used in this module.
type FsDebouncer = Debouncer<RecommendedWatcher, FileIdMap>;

/// Blacklisted path segments which are ignored by the watcher by default.
const BLACKLIST: [&str; 1] = [".git"];
/// The duration of time to debounce FS events.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(25);
/// The duration of time during which watcher events will be ignored following a build.
const WATCHER_COOLDOWN: Duration = Duration::from_secs(1);

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
    _debouncer: FsDebouncer,
    /// The application shutdown channel.
    shutdown: BroadcastStream<()>,
    /// Channel that is sent on whenever a build completes.
    build_done_tx: Option<broadcast::Sender<()>>,
    /// An instant used to track the last build time, used to implement the watcher cooldown
    /// to avoid infinite build loops.
    ///
    /// Ok, so why is this needed? As it turns out, `std::fs::copy` will trigger
    /// `EventKind::Modify(ModifyKind::Data(_))` FS events on the file which is being copied. A
    /// build cooldown period ensures that no FS events are processed until at least a duration
    /// of `WATCHER_COOLDOWN` has elapsed since the last build.
    last_build_finished: Instant,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(
        cfg: Arc<RtcWatch>,
        shutdown: broadcast::Sender<()>,
        build_done_tx: Option<broadcast::Sender<()>>,
        ws_protocol: Option<WsProtocol>,
    ) -> Result<Self> {
        // Create a channel for being able to listen for new paths to ignore while running.
        let (watch_tx, watch_rx) = mpsc::channel(1);
        let (build_tx, build_rx) = mpsc::channel(1);

        // Build the watcher.
        let _debouncer = build_watcher(watch_tx, cfg.paths.clone())?;

        // Build dependencies.
        let build = BuildSystem::new(cfg.build.clone(), Some(build_tx), ws_protocol).await?;
        Ok(Self {
            build,
            ignored_paths: cfg.ignored_paths.clone(),
            watch_rx,
            build_rx,
            _debouncer,
            shutdown: BroadcastStream::new(shutdown.subscribe()),
            build_done_tx,
            last_build_finished: Instant::now(),
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
    async fn handle_watch_event(&mut self, event: DebouncedEvent) {
        // There are various OS syscalls which can trigger FS changes, even though semantically no
        // changes were made. A notorious example which has plagued the trunk watcher
        // implementation is `std::fs::copy`, which will trigger watcher changes indicating
        // that file contents have been modified.
        //
        // Given the difficult nature of this issue, we opt for using a cooldown period. Any changes
        // events processed within the cooldown period following a build will be ignored.
        if Instant::now().duration_since(self.last_build_finished) <= WATCHER_COOLDOWN {
            // Purge any other events in the queue.
            while let Ok(_event) = self.watch_rx.try_recv() {}
            return;
        }

        // Check each path in the event for a match.
        match event.event.kind {
            EventKind::Modify(ModifyKind::Name(_) | ModifyKind::Data(_))
            | EventKind::Create(_)
            | EventKind::Remove(_) => (),
            _ => return,
        };
        let mut found_matching_path = false;
        for ev_path in &event.paths {
            let ev_path = match tokio::fs::canonicalize(&ev_path).await {
                Ok(ev_path) => ev_path,
                // Ignore errors here, as this would only take place for a resource which has
                // been removed, which will happen for each of our dist/.stage entries.
                Err(_) => continue,
            };

            // Check ignored paths.
            if ev_path.ancestors().any(|path| {
                self.ignored_paths
                    .iter()
                    .any(|ignored_path| ignored_path == path)
            }) {
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

            // If all of the above checks have passed, then we need to trigger a build.
            tracing::debug!("change detected in {:?} of type {:?}", ev_path, event.kind);
            found_matching_path = true;
        }

        // If a build is not needed, then return.
        if !found_matching_path {
            return;
        }

        // Else, time to trigger a build.
        let _res = self.build.build().await;
        self.last_build_finished = tokio::time::Instant::now();

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
) -> Result<FsDebouncer> {
    // Build the filesystem watcher & debouncer.
    let mut debouncer = new_debouncer(
        DEBOUNCE_DURATION,
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => events.into_iter().for_each(|event| {
                let _ = watch_tx.blocking_send(event);
            }),
            Err(errors) => errors
                .into_iter()
                .for_each(|err| tracing::warn!(error=?err, "error from filesystem watcher")),
        },
    )
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
