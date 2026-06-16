use crate::config::rt::RtcBuild;
use crate::config::types::CompressionAlgorithm;
use crate::pipelines::compress_dist;
use anyhow::{Context, Result};
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder};
use globset::{Glob, GlobSetBuilder};
use tokio::io::AsyncReadExt;

/// Build a test config rooted at a fresh tempdir with the given compression settings applied.
async fn test_cfg(algorithms: Vec<CompressionAlgorithm>) -> Result<(tempfile::TempDir, RtcBuild)> {
    let tmpdir = tempfile::tempdir().context("error building tempdir")?;
    let mut cfg = RtcBuild::new_test(tmpdir.path()).await?;
    cfg.compression.algorithms = algorithms;
    cfg.compression.min_size = 0;
    cfg.compression.min_ratio_percent = 100;
    Ok((tmpdir, cfg))
}

/// Write a file into the staging dist dir.
async fn write_staged(cfg: &RtcBuild, name: &str, bytes: &[u8]) -> Result<()> {
    let path = cfg.staging_dist.join(name);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

async fn read_staged(cfg: &RtcBuild, name: &str) -> Result<Vec<u8>> {
    Ok(tokio::fs::read(cfg.staging_dist.join(name)).await?)
}

fn exists(cfg: &RtcBuild, name: &str) -> bool {
    cfg.staging_dist.join(name).exists()
}

async fn gunzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    GzipDecoder::new(bytes).read_to_end(&mut out).await?;
    Ok(out)
}

async fn unbrotli(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    BrotliDecoder::new(bytes).read_to_end(&mut out).await?;
    Ok(out)
}

#[tokio::test]
async fn ok_roundtrip_gzip_and_brotli() -> Result<()> {
    let (_tmp, cfg) = test_cfg(vec![
        CompressionAlgorithm::Gzip,
        CompressionAlgorithm::Brotli,
    ])
    .await?;
    // Highly compressible content so the ratio gate is satisfied.
    let content = "trunk compresses assets\n".repeat(100).into_bytes();
    write_staged(&cfg, "index.html", &content).await?;

    compress_dist(&cfg).await?;

    let gz = read_staged(&cfg, "index.html.gz").await?;
    let br = read_staged(&cfg, "index.html.br").await?;
    anyhow::ensure!(
        gunzip(&gz).await? == content,
        "gzip sidecar did not roundtrip"
    );
    anyhow::ensure!(
        unbrotli(&br).await? == content,
        "brotli sidecar did not roundtrip"
    );
    Ok(())
}

#[tokio::test]
async fn skips_files_below_min_size() -> Result<()> {
    let (_tmp, mut cfg) = test_cfg(vec![CompressionAlgorithm::Gzip]).await?;
    cfg.compression.min_size = 1024;
    write_staged(&cfg, "small.txt", b"tiny").await?;

    compress_dist(&cfg).await?;

    anyhow::ensure!(
        !exists(&cfg, "small.txt.gz"),
        "expected no sidecar for a file below min_size"
    );
    Ok(())
}

#[tokio::test]
async fn skips_when_ratio_not_met() -> Result<()> {
    let (_tmp, mut cfg) = test_cfg(vec![CompressionAlgorithm::Gzip]).await?;
    // A ratio of 0% means the sidecar must be 0 bytes to be kept, which never happens.
    cfg.compression.min_ratio_percent = 0;
    write_staged(
        &cfg,
        "index.html",
        &"compressible\n".repeat(100).into_bytes(),
    )
    .await?;

    compress_dist(&cfg).await?;

    anyhow::ensure!(
        !exists(&cfg, "index.html.gz"),
        "expected no sidecar when the compression ratio gate is not met"
    );
    Ok(())
}

#[tokio::test]
async fn disabled_when_no_algorithms() -> Result<()> {
    let (_tmp, cfg) = test_cfg(vec![]).await?;
    write_staged(&cfg, "index.html", &"x".repeat(100).into_bytes()).await?;

    compress_dist(&cfg).await?;

    anyhow::ensure!(
        !exists(&cfg, "index.html.gz") && !exists(&cfg, "index.html.br"),
        "expected no sidecars when compression is disabled"
    );
    Ok(())
}

#[tokio::test]
async fn respects_include_and_exclude_globs() -> Result<()> {
    let (_tmp, mut cfg) = test_cfg(vec![CompressionAlgorithm::Gzip]).await?;
    let mut include = GlobSetBuilder::new();
    include.add(Glob::new("*.txt")?);
    cfg.compression.include = Some(include.build()?);
    let mut exclude = GlobSetBuilder::new();
    exclude.add(Glob::new("skip.txt")?);
    cfg.compression.exclude = Some(exclude.build()?);

    let content = "data\n".repeat(100).into_bytes();
    write_staged(&cfg, "keep.txt", &content).await?;
    write_staged(&cfg, "skip.txt", &content).await?;
    write_staged(&cfg, "image.png", &content).await?;

    compress_dist(&cfg).await?;

    anyhow::ensure!(
        exists(&cfg, "keep.txt.gz"),
        "included file should be compressed"
    );
    anyhow::ensure!(
        !exists(&cfg, "skip.txt.gz"),
        "excluded file should be skipped"
    );
    anyhow::ensure!(
        !exists(&cfg, "image.png.gz"),
        "non-included file should be skipped"
    );
    Ok(())
}

#[tokio::test]
async fn compresses_files_in_subdirectories() -> Result<()> {
    let (_tmp, cfg) = test_cfg(vec![CompressionAlgorithm::Gzip]).await?;
    let content = "nested\n".repeat(100).into_bytes();
    write_staged(&cfg, "assets/app.js", &content).await?;

    compress_dist(&cfg).await?;

    let gz = read_staged(&cfg, "assets/app.js.gz")
        .await
        .context("expected sidecar for nested file")?;
    anyhow::ensure!(
        gunzip(&gz).await? == content,
        "nested sidecar did not roundtrip"
    );
    Ok(())
}
