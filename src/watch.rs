use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_std::task::{spawn_blocking, JoinHandle};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::prelude::*;
use indicatif::ProgressBar;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

use crate::build::BuildSystem;
use crate::config::RtcWatch;

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    /// The build system progress bar for displaying the state of the build system overall.
    progress: ProgressBar,
    /// The build system.
    build: BuildSystem,
    /// The current vector of paths to be ignored.
    ignores: Vec<PathBuf>,
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

        // Process ignore list.
        let mut ignores = cfg.ignore.iter().try_fold(vec![], |mut acc, path| -> Result<Vec<PathBuf>> {
            let abs_path = path.canonicalize().map_err(|err| anyhow!("invalid path provided: {}", err))?;
            acc.push(abs_path);
            Ok(acc)
        })?;
        ignores.append(&mut vec![cfg.build.dist.clone()]);

        // Build the watcher.
        let _watcher = build_watcher(watch_tx)?;

        // Build dependencies.
        let build = BuildSystem::new(cfg.build.clone(), progress.clone(), Some(build_tx)).await?;
        Ok(Self {
            progress,
            build,
            ignores,
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
            futures::select! {
                ign_res = self.build_rx.next() => if let Some(ign) = ign_res {
                    self.update_ignore_list(ign);
                },
                ev_res = self.watch_rx.next() => if let Some(ev) = ev_res {
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
            Err(err) => return self.progress.println(format!("{}", err)),
        };

        for path in ev_path.ancestors() {
            if self.ignores.iter().map(|p| p.as_path()).any(|p| p == path) {
                return; // Don't emit a notification if ignored.
            }
        }

        if let Err(err) = self.build.build().await {
            self.progress.println(format!("{}", err));
        }
    }

    fn update_ignore_list(&mut self, path: PathBuf) {
        if !self.ignores.contains(&path) {
            self.ignores.push(path);
        }
    }
}

fn build_watcher(mut watch_tx: Sender<DebouncedEvent>) -> Result<(JoinHandle<()>, RecommendedWatcher)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = watcher(tx, std::time::Duration::from_secs(1)).context("failed to build file system watcher")?;
    watcher
        .watch(".", RecursiveMode::Recursive)
        .context("failed to watch CWD for file system changes")?;
    let handle = spawn_blocking(move || loop {
        if let Ok(event) = rx.recv() {
            let _ = watch_tx.try_send(event);
        }
    });
    Ok((handle, watcher))
}
