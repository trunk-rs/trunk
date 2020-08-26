use std::path::PathBuf;
use std::sync::mpsc::channel as std_channel;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_std::sync::channel;
use async_std::task::spawn_blocking;
use futures::stream::{FusedStream, StreamExt};
use notify::{Watcher, RecursiveMode, watcher};

use crate::common::get_cwd;
use crate::build::{BuildSystem, CargoManifest};

/// A watch system wrapping a build system and a watcher.
pub struct WatchSystem {
    build: BuildSystem,
    watcher: TrunkWatcher,
}

impl WatchSystem {
    /// Create a new instance.
    pub async fn new(target: PathBuf, release: bool, dist: PathBuf, public_url: String, ignore: Vec<PathBuf>) -> Result<Self> {
        // Process ignore list.
        let cwd = get_cwd().await?;
        let mut ignore = ignore.into_iter().try_fold(vec![], |mut acc, path| -> Result<Vec<PathBuf>> {
            let abs_path = path.canonicalize().map_err(|err| anyhow!("invalid path provided: {}", err))?;
            acc.push(abs_path);
            Ok(acc)
        })?;
        ignore.append(&mut vec![cwd.join("target"), cwd.join(&dist)]);

        // Build the watcher.
        let watcher = TrunkWatcher::new(ignore)?;

        // Perform an initial build.
        let manifest = CargoManifest::read_cwd_manifest().await?;
        let build = BuildSystem::new(manifest, target, release, dist, public_url).await?;
        Ok(Self{build, watcher})
    }

    /// Run a build.
    pub async fn build(&mut self) {
        if let Err(err) = self.build.build_app().await {
            eprintln!("{}", err);
        }
    }

    /// Run the watch system, responding to events and triggering builds.
    pub async fn run(mut self) {
        while let Some(_) = self.watcher.rx.next().await {
            if let Err(err) = self.build.build_app().await {
                eprintln!("{}", err);
            }
        }
    }
}

/// A watcher system for triggering Trunk builds.
struct TrunkWatcher {
    pub watcher: notify::RecommendedWatcher,
    pub rx: Box<dyn FusedStream<Item=()> + Send + Unpin>,
}

impl TrunkWatcher {
    /// Spawn a watcher to trigger builds as changes are detected on the filesystem.
    pub fn new(ignore: Vec<PathBuf>) -> Result<TrunkWatcher> {
        // Setup core watcher functionality.
        let (tx, rx) = std_channel();
        let mut watcher = watcher(tx, Duration::from_secs(1))
            .map_err(|err| anyhow!("error setting up watcher: {}", err))?;
        watcher.watch(".", RecursiveMode::Recursive)
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
                            Some(path) => eprintln!("watch error at {}\n{}", path.to_string_lossy(), err),
                            None => eprintln!("{}", err),
                        }
                        _ => continue,
                    }
                    Err(_) => return, // An error here indicates that the watcher has closed.
                }
            }
        });
        Ok(TrunkWatcher{watcher, rx: Box::new(async_rx.fuse())})
    }
}
