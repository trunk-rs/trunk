//! Build-time pre-compression of assets into sidecar files (e.g. `index.html.gz`).
//!
//! After all asset pipelines have written their output into the staging dist directory, this step
//! walks the directory and, for each configured algorithm, writes a compressed sidecar file next
//! to the original (e.g. `app.js` -> `app.js.gz`, `app.js.br`). Static file servers and CDNs can
//! then serve the precompressed variant based on the request's `Accept-Encoding` header.
//!
//! Compression is CPU-bound, so each (file, algorithm) job runs on the blocking thread pool via
//! [`tokio::task::spawn_blocking`], with up to one job per available core in flight at a time. A
//! live progress bar is shown per job (hidden automatically when stderr is not a terminal).

use crate::config::{
    rt::RtcBuild,
    types::{CompressionAlgorithm, CompressionLevel},
};
use anyhow::{Context, Result};
use flate2::{Compression, write::GzEncoder};
use futures_util::stream::{self, StreamExt, TryStreamExt};
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

/// The chunk size used when feeding data to the encoders, so progress bars advance smoothly.
const CHUNK_SIZE: usize = 256 * 1024;
/// Brotli window size (`lgwin`); 22 is the library default and a good general choice.
const BROTLI_WINDOW: u32 = 22;

/// A single compression job: one algorithm applied to one source file.
struct Job {
    /// The source file to read and compress.
    src: PathBuf,
    /// The sidecar file to write (e.g. `app.js.br`).
    sidecar: PathBuf,
    /// A short human label for the progress bar (e.g. `br app.js`).
    label: String,
    /// The original file size, used as the progress bar length.
    size: u64,
    algorithm: CompressionAlgorithm,
    level: CompressionLevel,
    /// Keep the sidecar only if its size is at most this percentage of the original.
    min_ratio_percent: u8,
}

/// Compress the assets in the staging dist directory according to the build's compression config.
///
/// This is a no-op when no compression algorithms are configured.
#[tracing::instrument(level = "trace", skip(cfg))]
pub async fn compress_dist(cfg: &RtcBuild) -> Result<()> {
    if !cfg.compression.enabled() {
        return Ok(());
    }

    let jobs = collect_jobs(cfg)
        .await
        .context("error scanning staging dist dir for compression")?;
    if jobs.is_empty() {
        return Ok(());
    }

    // One blocking job per available core keeps every core busy without oversubscribing.
    let concurrency = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let multi = MultiProgress::new();
    let overall = multi.add(ProgressBar::new(jobs.len() as u64));
    overall.set_style(overall_style());
    overall.set_prefix("Compressing assets");

    let results: Vec<(usize, u64)> = stream::iter(jobs)
        .map(|job| {
            let multi = multi.clone();
            let overall = overall.clone();
            async move {
                let bar = multi.insert_before(&overall, ProgressBar::new(job.size));
                bar.set_style(job_style());
                bar.set_message(job.label.clone());
                bar.enable_steady_tick(Duration::from_millis(120));

                let worker_bar = bar.clone();
                let result = tokio::task::spawn_blocking(move || run_job(job, &worker_bar))
                    .await
                    .context("compression task panicked")?;

                bar.finish_and_clear();
                overall.inc(1);
                result
            }
        })
        .buffer_unordered(concurrency)
        .try_collect()
        .await?;

    overall.finish_and_clear();

    let written: usize = results.iter().map(|(count, _)| count).sum();
    let saved: u64 = results.iter().map(|(_, bytes)| bytes).sum();
    tracing::info!(
        "compressed {written} asset sidecar(s), saved {}",
        HumanBytes(saved)
    );

    Ok(())
}

/// Walk the staging dist dir and build the list of compression jobs (after applying all filters).
async fn collect_jobs(cfg: &RtcBuild) -> Result<Vec<Job>> {
    let root = cfg.staging_dist.as_path();
    let mut jobs = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut entries = fs::read_dir(&dir)
            .await
            .map(ReadDirStream::new)
            .with_context(|| format!("error reading dir {dir:?}"))?;
        while let Some(entry) = entries.next().await {
            let entry = entry.with_context(|| format!("error reading entry in {dir:?}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .with_context(|| format!("error reading file type of {path:?}"))?;

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() || is_sidecar(&path) {
                continue;
            }

            let rel = path.strip_prefix(root).unwrap_or(&path);
            if !cfg.compression.matches(rel) {
                continue;
            }

            let size = entry
                .metadata()
                .await
                .with_context(|| format!("error reading metadata of {path:?}"))?
                .len();
            if size < cfg.compression.min_size {
                continue;
            }

            let rel_label = rel.to_string_lossy().into_owned();
            for &algorithm in &cfg.compression.algorithms {
                jobs.push(Job {
                    src: path.clone(),
                    sidecar: sidecar_path(&path, algorithm.extension()),
                    label: format!("{} {rel_label}", algorithm.extension()),
                    size,
                    algorithm,
                    level: cfg.compression.level,
                    min_ratio_percent: cfg.compression.min_ratio_percent,
                });
            }
        }
    }

    Ok(jobs)
}

/// Run a single compression job on a blocking thread. Returns `(sidecars_written, bytes_saved)`.
fn run_job(job: Job, bar: &ProgressBar) -> Result<(usize, u64)> {
    let data = std::fs::read(&job.src)
        .with_context(|| format!("error reading {:?} for compression", job.src))?;
    bar.set_length(data.len() as u64);

    let compressed = encode(job.algorithm, job.level, &data, bar)
        .with_context(|| format!("error compressing {:?} with {}", job.src, job.algorithm))?;

    // Only keep the sidecar if it is sufficiently smaller than the original.
    let max_size = data.len() * job.min_ratio_percent as usize / 100;
    if compressed.len() > max_size {
        return Ok((0, 0));
    }

    std::fs::write(&job.sidecar, &compressed)
        .with_context(|| format!("error writing compressed file {:?}", job.sidecar))?;
    let saved = (data.len() - compressed.len()) as u64;
    Ok((1, saved))
}

/// Compress `data` with the given algorithm and level, advancing `bar` as input is consumed.
fn encode(
    algorithm: CompressionAlgorithm,
    level: CompressionLevel,
    data: &[u8],
    bar: &ProgressBar,
) -> io::Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), gzip_level(level));
            write_chunked(&mut encoder, data, bar)?;
            encoder.finish()
        }
        CompressionAlgorithm::Brotli => {
            let mut encoder = brotli::CompressorWriter::new(
                Vec::new(),
                CHUNK_SIZE,
                brotli_quality(level),
                BROTLI_WINDOW,
            );
            write_chunked(&mut encoder, data, bar)?;
            Ok(encoder.into_inner())
        }
    }
}

/// Write `data` to `writer` in chunks, incrementing `bar` by the bytes consumed after each chunk.
fn write_chunked<W: Write>(writer: &mut W, data: &[u8], bar: &ProgressBar) -> io::Result<()> {
    for chunk in data.chunks(CHUNK_SIZE) {
        writer.write_all(chunk)?;
        bar.inc(chunk.len() as u64);
    }
    Ok(())
}

/// Map a [`CompressionLevel`] to a gzip (DEFLATE) level (0-9).
fn gzip_level(level: CompressionLevel) -> Compression {
    match level {
        CompressionLevel::Low => Compression::new(1),
        CompressionLevel::Medium => Compression::new(6),
        CompressionLevel::High => Compression::new(9),
    }
}

/// Map a [`CompressionLevel`] to a brotli quality (0-11).
fn brotli_quality(level: CompressionLevel) -> u32 {
    match level {
        CompressionLevel::Low => 2,
        CompressionLevel::Medium => 5,
        CompressionLevel::High => 11,
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

/// Progress bar style for the overall compression progress.
fn overall_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:.bold.cyan} {pos}/{len} {wide_bar:.cyan/blue} {elapsed}")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=> ")
}

/// Progress bar style for an individual compression job.
fn job_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "  {spinner:.green} {msg:<28} {bytes:>9}/{total_bytes:<9} {bar:20.green/dim}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_spinner())
}
