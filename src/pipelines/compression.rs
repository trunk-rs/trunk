//! Build-time pre-compression of assets into sidecar files (e.g. `index.html.gz`).
//!
//! After all asset pipelines have written their output into the staging dist directory, this step
//! walks the directory and, for each configured algorithm, writes a compressed sidecar file next
//! to the original (e.g. `app.js` -> `app.js.gz`, `app.js.br`). Static file servers and CDNs can
//! then serve the precompressed variant based on the request's `Accept-Encoding` header.

use crate::config::{rt::RtcBuild, types::CompressionAlgorithm};
use anyhow::{Context, Result};
use async_compression::tokio::write::{BrotliEncoder, GzipEncoder};
use futures_util::stream::{self, StreamExt, TryStreamExt};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::ReadDirStream;

/// The maximum number of files to compress concurrently.
const CONCURRENCY: usize = 8;

/// Compress the assets in the staging dist directory according to the build's compression config.
///
/// This is a no-op when no compression algorithms are configured.
#[tracing::instrument(level = "trace", skip(cfg))]
pub async fn compress_dist(cfg: &RtcBuild) -> Result<()> {
    if !cfg.compression.enabled() {
        return Ok(());
    }

    let root = cfg.staging_dist.clone();
    let files = collect_files(&root)
        .await
        .context("error scanning staging dist dir for compression")?;

    let written = stream::iter(files)
        .map(|path| compress_file(cfg, &root, path))
        .buffer_unordered(CONCURRENCY)
        .try_fold(0usize, |acc, n| async move { Ok(acc + n) })
        .await?;

    tracing::info!("wrote {written} compressed asset sidecar(s)");
    Ok(())
}

/// Recursively collect all regular files under `root`.
async fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut entries = fs::read_dir(&dir)
            .await
            .map(ReadDirStream::new)
            .with_context(|| format!("error reading dir {dir:?}"))?;
        while let Some(entry) = entries.next().await {
            let entry = entry.with_context(|| format!("error reading entry in {dir:?}"))?;
            let file_type = entry
                .file_type()
                .await
                .with_context(|| format!("error reading file type of {:?}", entry.path()))?;
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }

    Ok(files)
}

/// Compress a single file with all configured algorithms, returning the number of sidecars written.
async fn compress_file(cfg: &RtcBuild, root: &Path, path: PathBuf) -> Result<usize> {
    let rel = path.strip_prefix(root).unwrap_or(&path);

    // Never compress an existing sidecar.
    if is_sidecar(&path) {
        return Ok(0);
    }

    // Apply include/exclude globs.
    if !cfg.compression.matches(rel) {
        return Ok(0);
    }

    let bytes = fs::read(&path)
        .await
        .with_context(|| format!("error reading {path:?} for compression"))?;

    // Skip files below the configured size threshold.
    if (bytes.len() as u64) < cfg.compression.min_size {
        return Ok(0);
    }

    // Largest allowed sidecar size: original * min_ratio_percent / 100.
    let max_size = bytes.len() * cfg.compression.min_ratio_percent as usize / 100;

    let mut written = 0;
    for algorithm in &cfg.compression.algorithms {
        let compressed = encode(*algorithm, &bytes)
            .await
            .with_context(|| format!("error compressing {path:?} with {algorithm}"))?;

        // Only keep the sidecar if it is sufficiently smaller than the original.
        if compressed.len() > max_size {
            continue;
        }

        let sidecar = sidecar_path(&path, algorithm.extension());
        fs::write(&sidecar, &compressed)
            .await
            .with_context(|| format!("error writing compressed file {sidecar:?}"))?;
        written += 1;
    }

    Ok(written)
}

/// Compress `bytes` with the given algorithm.
async fn encode(algorithm: CompressionAlgorithm, bytes: &[u8]) -> Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::Gzip => {
            let mut encoder = GzipEncoder::new(Vec::new());
            encoder.write_all(bytes).await?;
            encoder.shutdown().await?;
            Ok(encoder.into_inner())
        }
        CompressionAlgorithm::Brotli => {
            let mut encoder = BrotliEncoder::new(Vec::new());
            encoder.write_all(bytes).await?;
            encoder.shutdown().await?;
            Ok(encoder.into_inner())
        }
    }
}

/// Build the sidecar path by appending `.<ext>` to the original file name.
fn sidecar_path(path: &Path, ext: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}

/// Whether the path already looks like a compressed sidecar produced by this step.
fn is_sidecar(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("gz") | Some("br")
    )
}
