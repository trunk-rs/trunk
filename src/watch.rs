use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use indicatif::ProgressBar;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::build::BuildSystem;
use crate::config::RtcWatch;

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system progress bar for displaying the state of the build system overall.
    progress: ProgressBar,
    /// The build system.
    build: BuildSystem,
    /// The current vector of paths to be ignored.
    ignored_paths: Vec<PathBuf>,
    /// A channel of FS watch events.
    watch_rx: Receiver<DebouncedEvent>,
    /// A channel of new paths to ignore from the build system.
    build_rx: Receiver<PathBuf>,
    /// The watch system used for watching the filesystem.
    _watcher: (JoinHandle<()>, RecommendedWatcher),
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(cfg: Arc<RtcWatch>, progress: ProgressBar) -> Result<Self> {
        // Create a channel for being able to listen for new paths to ignore while running.
        let (watch_tx, watch_rx) = channel(1);
        let (build_tx, build_rx) = channel(1);

        // Build the watcher.
        let _watcher = build_watcher(watch_tx, cfg.paths.clone())?;

        // Build dependencies.
        let build = BuildSystem::new(cfg.build.clone(), progress.clone(), Some(build_tx)).await?;
        Ok(Self {
            progress,
            build,
            ignored_paths: cfg.ignored_paths.clone(),
            watch_rx,
            build_rx,
            _watcher,
        })
    }

    /// Run a build.
    pub async fn build(&mut self) {
        if let Err(err) = self.build.build().await {
            // NOTE WELL: we use debug formatting here to ensure the error chain is displayed.
            self.progress.println(format!("{:?}", err));
        }
    }

    /// Run the watch system, responding to events and triggering builds.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // NOTE WELL: as of Trunk 0.8.0, this channel is only ever used to ignore the
                // cargo target dir, which is determined dynamically at runtime. The path MUST
                // be the canonical path when it is sent over this channel from the build system.
                ign_res = self.build_rx.recv() => if let Some(ign) = ign_res {
                    self.update_ignore_list(ign);
                },
                ev_res = self.watch_rx.recv() => if let Some(ev) = ev_res {
                    self.handle_watch_event(ev).await;
                },
            }
        }
    }

    async fn handle_watch_event(&mut self, event: DebouncedEvent) {
        let mut ev_path = match event {
            DebouncedEvent::Create(path) | DebouncedEvent::Write(path) | DebouncedEvent::Remove(path) | DebouncedEvent::Rename(_, path) => path,
            _ => return,
        };

        ev_path = match ev_path.canonicalize() {
            Ok(path) => path,
            // Ignore errors here, as this would only take place for a resource which has
            // been removed, which will happen for each of our dist/.stage entries.
            Err(_) => return,
        };

        if ev_path
            .ancestors()
            .any(|path| self.ignored_paths.iter().any(|ignored_path| ignored_path == path))
        {
            return; // Don't emit a notification if path is ignored.
        }

        if let Err(err) = self.build.build().await {
            self.progress.println(format!("{}", err));
        }
    }

    fn update_ignore_list(&mut self, path: PathBuf) {
        if !self.ignored_paths.contains(&path) {
            self.ignored_paths.push(path);
        }
    }
}

fn build_watcher(watch_tx: Sender<DebouncedEvent>, paths: Vec<PathBuf>) -> Result<(JoinHandle<()>, RecommendedWatcher)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = watcher(tx, std::time::Duration::from_secs(1)).context("failed to build file system watcher")?;

    // Create a recursive watcher on each of the given paths.
    // NOTE WELL: it is expected that all given paths are canonical. The Trunk config
    // system currently ensures that this is true for all data coming from the
    // RtcBuild/RtcWatch/RtcServe/&c runtime config objects.
    for path in paths {
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .context(format!("failed to watch {:?} for file system changes", path))?;
    }

    let handle = spawn_blocking(move || loop {
        if let Ok(event) = rx.recv() {
            let _ = watch_tx.try_send(event);
        }
    });

    Ok((handle, watcher))
}
