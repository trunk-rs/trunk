use std::path::PathBuf;
use std::sync::mpsc::channel as std_channel;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_std::sync::channel;
use async_std::task::spawn_blocking;
use console::Emoji;
use futures::stream::{FusedStream, StreamExt};
use indicatif::ProgressBar;
use notify::{watcher, RecursiveMode, Watcher};

use crate::build::BuildSystem;
use crate::common::get_cwd;
use crate::config::RtcWatch;

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    build: BuildSystem,
    watcher: TrunkWatcher,
    progress: ProgressBar,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(cfg: Arc<RtcWatch>) -> Result<Self> {
        // Process ignore list.
        let cwd = get_cwd().await?;
        let mut ignore = cfg.ignore.iter().try_fold(vec![], |mut acc, path| -> Result<Vec<PathBuf>> {
            let abs_path = path.canonicalize().map_err(|err| anyhow!("invalid path provided: {}", err))?;
            acc.push(abs_path);
            Ok(acc)
        })?;
        ignore.append(&mut vec![cwd.join("target"), cwd.join(&cfg.build.dist)]);

        // Perform an initial build.
        let build = BuildSystem::new(cfg.build.clone()).await?;
        let progress = build.get_progress_handle();

        let watcher = TrunkWatcher::new(ignore, progress.clone())?;
        Ok(Self { build, watcher, progress })
    }

    /// Run a build.
    pub async fn build(&mut self) {
        if let Err(err) = self.build.build(true).await {
            self.progress.println(format!("{}", err));
        }
    }

    /// Run the watch system, responding to events and triggering builds.
    pub async fn run(mut self) {
        while self.watcher.rx.next().await.is_some() {
            if let Err(err) = self.build.build(true).await {
                self.progress.println(format!("{}", err));
            }
        }
    }

    /// Get a handle to the progress / terminal system.
    pub fn get_progress_handle(&self) -> ProgressBar {
        self.build.get_progress_handle()
    }
}

/// A watcher system for triggering Trunk builds.
struct TrunkWatcher {
    #[allow(dead_code)]
    watcher: notify::RecommendedWatcher,
    rx: Box<dyn FusedStream<Item = ()> + Send + Unpin>,
}

impl TrunkWatcher {
    /// Spawn a watcher to trigger builds as changes are detected on the filesystem.
    pub fn new(ignore: Vec<PathBuf>, progress: ProgressBar) -> Result<TrunkWatcher> {
        // Setup core watcher functionality.
        let (tx, rx) = std_channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).map_err(|err| anyhow!("error setting up watcher: {}", err))?;
        watcher
            .watch(".", RecursiveMode::Recursive)
            .map_err(|err| anyhow!("error watching current directory: {}", err))?;

        // Setup notification bridge between sync & async land.
        // NOTE: once notify@v5 lands, we should be able to simplify this quite a lot.
        let (async_tx, async_rx) = channel(1);
        spawn_blocking(move || {
            use notify::DebouncedEvent as Event;
            'outer: loop {
                match rx.recv() {
                    Ok(event) => match event {
                        Event::Create(path) | Event::Write(path) | Event::Remove(path) | Event::Rename(_, path) => {
                            for ancestor in path.ancestors() {
                                if ignore.contains(&ancestor.into()) {
                                    continue 'outer;
                                }
                            }
                            let _ = async_tx.try_send(());
                        }
                        Event::Error(err, path_opt) => match path_opt {
                            Some(path) => progress.println(&format!("{}watch error at {}\n{}", Emoji("🚫 ", ""), path.to_string_lossy(), err)),
                            None => progress.println(format!("{}", err)),
                        },
                        _ => continue,
                    },
                    Err(_) => return, // An error here indicates that the watcher has closed.
                }
            }
        });
        Ok(TrunkWatcher {
            watcher,
            rx: Box::new(async_rx.fuse()),
        })
    }
}
