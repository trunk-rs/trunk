//! Build system & asset pipelines.

use std::ffi::OsString;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use tokio::fs;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use tokio::time::sleep;
use tokio_stream::wrappers::ReadDirStream;

use crate::common::{BUILDING, ERROR, SUCCESS, remove_dir_all};
use crate::config::{STAGE_DIR, rt::RtcBuild, types::WsProtocol};
use crate::pipelines::HtmlPipeline;

pub type BuildResult = Result<()>;

const BUILD_LOCK_SUFFIX: &str = ".trunk-lock";
const BUILD_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(50);

enum LockAttempt {
    /// The file now owns the exclusive lock.
    Acquired(File),
    /// Another process owns the lock; return the file so the caller can retry.
    WouldBlock(File),
    /// Lock acquisition failed for a reason other than contention.
    Error(std::io::Error),
}

/// Acquire an exclusive lock for the distribution directory.
///
/// The lock file is a sibling of the distribution directory so that applying or cleaning a
/// distribution cannot remove it while another process is waiting for the lock.
async fn acquire_build_lock(final_dist: &Path) -> Result<File> {
    let mut lock_path = OsString::from(final_dist.as_os_str());
    lock_path.push(BUILD_LOCK_SUFFIX);
    let lock_path = PathBuf::from(lock_path);

    // Opening a file may block on filesystem I/O, so keep it off Tokio's worker threads.
    let open_lock_path = lock_path.clone();
    let mut file = spawn_blocking(move || {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(open_lock_path)
    })
    .await
    .context("error awaiting build lock file open")?
    .with_context(|| format!("error opening build lock file: {}", lock_path.display()))?;

    let mut waiting = false;
    loop {
        // File locking is synchronous in the standard library. Use a nonblocking attempt on
        // Tokio's blocking pool instead of awaiting one indefinitely blocking lock call. Besides
        // keeping the async worker free, this makes the wait cancellation-safe: dropping this
        // future can leave at most one short try_lock syscall running.
        let attempt = spawn_blocking(move || match file.try_lock() {
            Ok(()) => LockAttempt::Acquired(file),
            Err(TryLockError::WouldBlock) => LockAttempt::WouldBlock(file),
            Err(TryLockError::Error(err)) => LockAttempt::Error(err),
        })
        .await
        .context("error awaiting build lock acquisition")?;

        match attempt {
            LockAttempt::Acquired(file) => {
                if waiting {
                    tracing::info!("acquired build lock: {}", lock_path.display());
                }
                return Ok(file);
            }
            LockAttempt::WouldBlock(returned_file) => {
                if !waiting {
                    tracing::info!("waiting for build lock: {}", lock_path.display());
                    waiting = true;
                }
                file = returned_file;
                // Yield asynchronously before scheduling the next nonblocking lock attempt.
                sleep(BUILD_LOCK_RETRY_INTERVAL).await;
            }
            LockAttempt::Error(err) => {
                return Err(err).with_context(|| {
                    format!("error acquiring build lock: {}", lock_path.display())
                });
            }
        }
    }
}

/// A system used for building a Rust WASM app & bundling its assets.
///
/// This unit of data should be used throughout the system for driving build processes and
/// bundling tasks. Different CLI commands which need to trigger builds in some way should
/// be able to gather the needed data to create an instance of this struct, and then the various
/// build routines can be cleanly abstracted away form any specific CLI endpoints.
pub struct BuildSystem {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// HTML build pipeline.
    html_pipeline: Arc<HtmlPipeline>,
}

impl BuildSystem {
    /// Create a new instance from the raw components.
    ///
    /// Reducing the number of assumptions here should help us to stay flexible when adding new
    /// commands, refactoring and the like.
    pub async fn new(
        cfg: Arc<RtcBuild>,
        ignore_chan: Option<mpsc::Sender<Vec<PathBuf>>>,
        ws_protocol: Option<WsProtocol>,
    ) -> Result<Self> {
        let html_pipeline = Arc::new(HtmlPipeline::new(cfg.clone(), ignore_chan, ws_protocol)?);
        Ok(Self { cfg, html_pipeline })
    }

    /// Build the application described in the given build data.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn build(&mut self) -> Result<()> {
        let _build_lock = acquire_build_lock(&self.cfg.final_dist).await?;
        tracing::info!("{}starting build", BUILDING);
        let res = self.do_build().await;
        match res {
            Ok(_) => {
                tracing::info!("{}success", SUCCESS);
                Ok(())
            }
            Err(err) => {
                tracing::error!("{}error\n{:?}", ERROR, err);
                Err(err)
            }
        }
    }

    /// Internal business logic of `build`.
    async fn do_build(&mut self) -> Result<()> {
        // Ensure the output dist directories are in place.
        fs::create_dir_all(self.cfg.final_dist.as_path())
            .await
            .with_context(|| {
                format!(
                    "error creating build environment directory: {}",
                    self.cfg.final_dist.display()
                )
            })?;

        self.prepare_staging_dist()
            .await
            .context("error preparing build environment")?;

        // Spawn the source HTML pipeline. This will spawn all other pipelines derived from
        // the source HTML, and will ultimately generate and write the final HTML.
        self.html_pipeline
            .clone()
            .spawn()
            .await
            .context("error joining HTML pipeline")?
            // we name if "build" pipeline here, was that's what it has become, and
            // what makes more sense to the user
            .context("error from build pipeline")?;

        // Move distribution from staging dist to final dist
        self.finalize_dist()
            .await
            .context("error applying built distribution")?;
        Ok(())
    }

    /// Creates a "staging area" (dist/.stage) for storing intermediate build results.
    async fn prepare_staging_dist(&self) -> Result<()> {
        // Prepare staging area in which we will assemble the latest build
        let staging_dist = self.cfg.staging_dist.as_path();

        // Clean staging area, if applicable
        remove_dir_all(staging_dist.into()).await.with_context(|| {
            format!(
                "error cleaning staging dist dir: {}",
                staging_dist.display()
            )
        })?;

        fs::create_dir_all(staging_dist).await.with_context(|| {
            format!(
                "error creating build environment directory: {}",
                staging_dist.display()
            )
        })?;

        Ok(())
    }

    /// Moves the contents of dist/.stage into dist, signifying the application
    /// of a successful build. Also removes dist/.stage afterwards.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn finalize_dist(&self) -> Result<()> {
        let staging_dist = self.cfg.staging_dist.clone();
        tracing::info!("applying new distribution");

        // Build succeeded, so delete everything in `dist`, move everything
        // from `dist/.stage` to `dist`, and then delete `dist/.stage`.
        self.clean_final().await?;
        self.move_stage_to_final().await?;
        fs::remove_dir(staging_dist)
            .await
            .context("error deleting staging dist dir")?;

        Ok(())
    }

    /// Move contents of stage dir to final dist dir.
    async fn move_stage_to_final(&self) -> Result<()> {
        let final_dist = self.cfg.final_dist.clone();
        let staging_dist = self.cfg.staging_dist.clone();

        let mut entries = fs::read_dir(&staging_dist)
            .await
            .map(ReadDirStream::new)
            .context("error reading staging dist dir")?;
        while let Some(entry) = entries.next().await {
            let entry = entry.context("error reading contents of staging dist dir")?;
            let target_path = final_dist.join(entry.file_name());

            fs::rename(entry.path(), &target_path)
                .await
                .with_context(|| {
                    format!("error moving {:?} to {:?}", entry.path(), target_path)
                })?;
        }
        Ok(())
    }

    /// Clean the contents of the final dist dir.
    async fn clean_final(&self) -> Result<()> {
        let final_dist = self.cfg.final_dist.clone();

        let mut entries = fs::read_dir(&final_dist)
            .await
            .map(ReadDirStream::new)
            .context("error reading final dist dir")?;
        while let Some(entry) = entries.next().await {
            let entry = entry.context("error reading contents of final dist dir")?;
            if entry.file_name() == STAGE_DIR {
                continue;
            }

            let file_type = entry
                .file_type()
                .await
                .context("error reading metadata of file in final dist dir")?;
            if file_type.is_dir() {
                remove_dir_all(entry.path())
                    .await
                    .context("error cleaning final dist")?;
            } else if file_type.is_symlink() || file_type.is_file() {
                fs::remove_file(entry.path())
                    .await
                    .context("error cleaning final dist")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::acquire_build_lock;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn build_lock_serializes_same_dist() {
        let tmpdir = tempfile::tempdir().expect("must create temp directory");
        let dist = tmpdir.path().join("dist");

        let first = acquire_build_lock(&dist)
            .await
            .expect("must acquire first lock");
        let second = acquire_build_lock(&dist);

        assert!(
            timeout(Duration::from_millis(150), second).await.is_err(),
            "second lock must wait"
        );

        drop(first);

        timeout(Duration::from_secs(1), acquire_build_lock(&dist))
            .await
            .expect("lock acquisition must not time out")
            .expect("must acquire lock after release");
    }

    #[tokio::test]
    async fn build_locks_are_scoped_by_dist() {
        let tmpdir = tempfile::tempdir().expect("must create temp directory");
        let first_dist = tmpdir.path().join("dist-a");
        let second_dist = tmpdir.path().join("dist-b");

        let _first = acquire_build_lock(&first_dist)
            .await
            .expect("must acquire first lock");

        timeout(Duration::from_secs(1), acquire_build_lock(&second_dist))
            .await
            .expect("independent lock acquisition must not time out")
            .expect("must acquire independent lock");
    }

    #[tokio::test]
    async fn build_lock_error_contains_path() {
        let tmpdir = tempfile::tempdir().expect("must create temp directory");
        let dist = tmpdir.path().join("missing").join("dist");
        let lock_path = format!("{}{}", dist.display(), super::BUILD_LOCK_SUFFIX);

        let error = acquire_build_lock(&dist)
            .await
            .expect_err("missing parent must fail");

        assert!(
            format!("{error:#}").contains(&lock_path),
            "error must contain lock path"
        );
    }
}
