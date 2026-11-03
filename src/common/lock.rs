use anyhow::{Context, Result};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::Path,
    time::Duration,
};
use tokio::{task::spawn_blocking, time::sleep};

pub(crate) const BUILD_LOCK_FILE: &str = ".trunk-lock";
const BUILD_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(50);

enum LockAttempt {
    /// The file now owns the exclusive lock.
    Acquired(File),
    /// Another process owns the lock; return the file so the caller can retry.
    WouldBlock(File),
    /// Lock acquisition failed for a reason other than contention.
    Error(std::io::Error),
}

/// Acquire dist before target; never acquire dist while holding a target lock.
pub(crate) async fn acquire_build_lock(final_dist: &Path) -> Result<File> {
    let lock_path = final_dist.join(BUILD_LOCK_FILE);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shared_target_serializes_processing_and_cache() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = dir.path().join("app.js");
        let cache = super::super::cache::Cache::new(dir.path().join("cache.json"));
        let first_ready = tokio::sync::Notify::new();
        let first = async {
            let _guard = acquire_build_lock(dir.path()).await?;
            first_ready.notify_one();
            cache
                .try_get(&[super::super::cache::CacheInput::Value("A")], || async {
                    tokio::fs::write(&output, b"A").await?;
                    tokio::task::yield_now().await;
                    Ok(vec![output.clone()])
                })
                .await?;
            assert_eq!(tokio::fs::read(&output).await?, b"A");
            anyhow::Ok(())
        };
        let second = async {
            first_ready.notified().await;
            let _guard = acquire_build_lock(dir.path()).await?;
            cache
                .try_get(&[super::super::cache::CacheInput::Value("B")], || async {
                    tokio::fs::write(&output, b"B").await?;
                    Ok(vec![output.clone()])
                })
                .await?;
            assert_eq!(tokio::fs::read(&output).await?, b"B");
            anyhow::Ok(())
        };
        let (a, b) = tokio::join!(first, second);
        a?;
        b?;
        Ok(())
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn child_observes_parent_lock() -> Result<()> {
        const ENV: &str = "TRUNK_TEST_LOCK_DIRECTORY";
        if let Some(path) = std::env::var_os(ENV) {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(Path::new(&path).join(BUILD_LOCK_FILE))?;
            assert!(matches!(file.try_lock(), Err(TryLockError::WouldBlock)));
            return Ok(());
        }
        let dir = tempfile::tempdir()?;
        let runtime = tokio::runtime::Runtime::new()?;
        let _guard = runtime.block_on(acquire_build_lock(dir.path()))?;
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "common::lock::process_tests::child_observes_parent_lock",
            ])
            .env(ENV, dir.path())
            .status()?;
        assert!(status.success());
        Ok(())
    }
}
