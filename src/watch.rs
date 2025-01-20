use crate::{
    build::{BuildResult, BuildSystem},
    config::{rt::RtcWatch, types::WsProtocol},
    ws,
};
use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use notify::{
    event::{MetadataKind, ModifyKind},
    EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};
use notify_debouncer_full::{
    new_debouncer_opt, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use std::path::Path;
use std::{fmt::Write, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, mpsc, watch, Mutex},
    time::Instant,
};
use tokio_stream::wrappers::BroadcastStream;

pub enum FsDebouncer {
    Default(Debouncer<RecommendedWatcher, FileIdMap>),
    Polling(Debouncer<PollWatcher, FileIdMap>),
}

impl FsDebouncer {
    pub fn watch(
        &mut self,
        path: impl AsRef<Path>,
        recursive_mode: RecursiveMode,
    ) -> notify::Result<()> {
        match self {
            Self::Default(deb) => deb.watch(path, recursive_mode),
            Self::Polling(deb) => deb.watch(path, recursive_mode),
        }
    }
}

/// Blacklisted path segments which are ignored by the watcher by default.
const BLACKLIST: [&str; 2] = [".git", ".DS_Store"];
/// The duration of time to debounce FS events.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(25);
/// The duration of time during which watcher events will be ignored following a build.
///
/// There are various OS syscalls which can trigger FS changes, even though semantically
/// no changes were made. A notorious example which has plagued the trunk
/// watcher implementation is `std::fs::copy`, which will trigger watcher
/// changes indicating that file contents have been modified.
///
/// Given the difficult nature of this issue, we opt for using a cooldown period. Any
/// changes events processed within the cooldown period following a build
/// will be ignored.
const WATCHER_COOLDOWN: Duration = Duration::from_secs(1);

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system.
    build: Arc<Mutex<BuildSystem>>,
    /// The current vector of paths to be ignored.
    ignored_paths: Vec<PathBuf>,
    /// A channel of FS watch events.
    watch_rx: mpsc::Receiver<DebouncedEvent>,
    /// A channel of new paths to ignore from the build system.
    ignore_rx: mpsc::Receiver<PathBuf>,
    /// A sender to notify the end of a build.
    build_tx: mpsc::Sender<BuildResult>,
    /// A channel to receive the end of a build.
    build_rx: mpsc::Receiver<BuildResult>,
    /// The watch system used for watching the filesystem.
    _debouncer: FsDebouncer,
    /// The application shutdown channel.
    shutdown: BroadcastStream<()>,
    /// Channel to communicate with the client socket
    ws_state: Option<watch::Sender<ws::State>>,
    /// Timestamp the last build was started.
    last_build_started: Instant,
    /// An instant used to track the last build time, used to implement the watcher cooldown
    /// to avoid infinite build loops.
    ///
    /// Ok, so why is this needed? As it turns out, `std::fs::copy` will trigger
    /// `EventKind::Modify(ModifyKind::Data(_))` FS events on the file which is being copied. A
    /// build cooldown period ensures that no FS events are processed until at least a duration
    /// of `WATCHER_COOLDOWN` has elapsed since the last build.
    last_build_finished: Instant,
    /// The timestamp of the last accepted change event.
    last_change: Instant,
    /// The cooldown for the watcher. [`None`] disables the cooldown.
    watcher_cooldown: Option<Duration>,
    /// Clear the screen before each run
    clear_screen: bool,
    /// Don't send build errors to the frontend.
    no_error_reporting: bool,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(
        cfg: Arc<RtcWatch>,
        shutdown: broadcast::Sender<()>,
        ws_state: Option<watch::Sender<ws::State>>,
        ws_protocol: Option<WsProtocol>,
    ) -> Result<Self> {
        // Create a channel for being able to listen for new paths to ignore while running.
        let (watch_tx, watch_rx) = mpsc::channel(1);
        let (ignore_tx, ignore_rx) = mpsc::channel(1);
        let (build_tx, build_rx) = mpsc::channel(1);

        // Build the watcher.
        let _debouncer = build_watcher(watch_tx, cfg.paths.clone(), cfg.poll)?;

        // Cooldown
        let watcher_cooldown = cfg.enable_cooldown.then_some(WATCHER_COOLDOWN);
        tracing::debug!(
            "Build cooldown: {:?}",
            watcher_cooldown.map(humantime::Duration::from)
        );

        // Build dependencies.
        let build = Arc::new(Mutex::new(
            BuildSystem::new(cfg.build.clone(), Some(ignore_tx), ws_protocol).await?,
        ));
        Ok(Self {
            build,
            ignored_paths: cfg.ignored_paths.clone(),
            watch_rx,
            ignore_rx,
            build_rx,
            build_tx,
            _debouncer,
            shutdown: BroadcastStream::new(shutdown.subscribe()),
            ws_state,
            last_build_started: Instant::now(),
            last_build_finished: Instant::now(),
            last_change: Instant::now(),
            watcher_cooldown,
            clear_screen: cfg.clear_screen,
            no_error_reporting: cfg.no_error_reporting,
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
                Some(ign) = self.ignore_rx.recv() => self.update_ignore_list(ign),
                Some(ev) = self.watch_rx.recv() => self.handle_watch_event(ev).await,
                Some(build) = self.build_rx.recv() => self.build_complete(build).await,
                _ = self.shutdown.next() => break, // Any event, even a drop, will trigger shutdown.
            }
        }

        tracing::debug!("watcher system has shut down");
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn build_complete(&mut self, build_result: Result<(), anyhow::Error>) {
        tracing::debug!("Build reported completion");

        // record last finish timestamp
        self.last_build_finished = Instant::now();

        if let Some(tx) = &mut self.ws_state {
            match build_result {
                Ok(()) => {
                    let _ = tx.send_replace(ws::State::Ok);
                }
                Err(err) => {
                    if !self.no_error_reporting {
                        let _ = tx.send_replace(ws::State::Failed {
                            reason: build_error_reason(err),
                        });
                    }
                }
            }
        }

        // check we need another build
        self.check_spawn_build().await;
    }

    /// check if a build is active
    fn is_build_active(&self) -> bool {
        self.last_build_started > self.last_build_finished
    }

    /// Spawn a new build
    async fn spawn_build(&mut self) {
        self.last_build_started = Instant::now();

        let build = self.build.clone();
        let build_tx = self.build_tx.clone();

        tokio::spawn(async move {
            // run the build
            let result = build.lock().await.build().await;
            // report the result
            build_tx.send(result).await
        });
    }

    async fn check_spawn_build(&mut self) {
        if self.last_change <= self.last_build_started {
            tracing::trace!("No changes since the last build was started");
            return;
        }

        tracing::debug!("Changes since the last build was started, checking cooldown");

        if let Some(cooldown) = self.watcher_cooldown {
            let time_since_last_build = self
                .last_change
                .saturating_duration_since(self.last_build_finished);
            if time_since_last_build < cooldown {
                tracing::debug!(
                    "Cooldown is still active: {} remaining",
                    humantime::Duration::from(cooldown - time_since_last_build)
                );
                return;
            }
        }

        if self.clear_screen {
            // This first message will not be seen if the clear screen worked.
            tracing::trace!("Clear screen is enabled, clearing the screen");
            let term = console::Term::stdout();
            if let Err(err) = term.clear_screen() {
                tracing::error!("Unable to clear the screen due to error: #{err}");
            } else {
                tracing::trace!("Clear screen is enabled, cleared the screen");
            }
        }
        self.spawn_build().await;
    }

    #[tracing::instrument(level = "trace", skip(self, event))]
    async fn handle_watch_event(&mut self, event: DebouncedEvent) {
        tracing::trace!(
            "change detected in {:?} of type {:?}",
            event.paths,
            event.kind
        );

        if !self.is_event_relevant(&event).await {
            tracing::trace!("Event not relevant, skipping");
            return;
        }

        // record time of the last accepted change
        self.last_change = Instant::now();

        if self.is_build_active() {
            tracing::debug!("Build is active, postponing start");
            return;
        }

        // Else, time to trigger a build.
        self.check_spawn_build().await;
    }

    async fn is_event_relevant(&self, event: &DebouncedEvent) -> bool {
        // Check each path in the event for a match.
        match event.event.kind {
            EventKind::Modify(
                ModifyKind::Name(_)
                | ModifyKind::Data(_)
                | ModifyKind::Metadata(MetadataKind::WriteTime)
                | ModifyKind::Any,
            )
            | EventKind::Create(_)
            | EventKind::Remove(_) => (),
            _ => return false,
        };

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
            tracing::debug!("accepted change in {:?} of type {:?}", ev_path, event.kind);
            // But we can return early, as we don't need to check the remaining changes
            return true;
        }

        false
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

fn new_debouncer<T: Watcher>(
    watch_tx: mpsc::Sender<DebouncedEvent>,
    config: Option<notify::Config>,
) -> Result<Debouncer<T, FileIdMap>> {
    new_debouncer_opt::<_, T, FileIdMap>(
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
        FileIdMap::new(),
        config.unwrap_or_default(),
    )
    .context("failed to build file system watcher")
}

/// Build a FS watcher, when the watcher is dropped, it will stop watching for events.
fn build_watcher(
    watch_tx: mpsc::Sender<DebouncedEvent>,
    paths: Vec<PathBuf>,
    poll: Option<Duration>,
) -> Result<FsDebouncer> {
    // Build the filesystem watcher & debouncer.

    if let Some(duration) = poll {
        tracing::info!(
            "Running in polling mode: {}",
            humantime::Duration::from(duration)
        );
    }

    let mut debouncer = match poll {
        None => FsDebouncer::Default(new_debouncer::<RecommendedWatcher>(watch_tx, None)?),
        Some(duration) => FsDebouncer::Polling(new_debouncer::<PollWatcher>(
            watch_tx,
            Some(notify::Config::default().with_poll_interval(duration)),
        )?),
    };

    // Create a recursive watcher on each of the given paths.
    // NOTE WELL: it is expected that all given paths are canonical. The Trunk config
    // system currently ensures that this is true for all data coming from the
    // RtcBuild/RtcWatch/RtcServe/&c runtime config objects.
    for path in paths {
        debouncer
            .watch(&path, RecursiveMode::Recursive)
            .context(format!(
                "failed to watch {:?} for file system changes",
                path
            ))?;
    }

    Ok(debouncer)
}

fn build_error_reason(error: anyhow::Error) -> String {
    let mut result = error.to_string();
    result.push_str("\n\n");

    let mut i = 0usize;
    let mut next = error.source();
    while let Some(current) = next {
        if i == 0 {
            writeln!(&mut result, "Caused by:").unwrap();
        }
        writeln!(&mut result, "\t{i}: {current}").unwrap();

        i += 1;
        next = current.source();
    }

    result
}
