use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::config::rt::RtcBuild;
use crate::pipelines::copy_file::*;
use crate::pipelines::ATTR_HREF;

/// A fixture for setting up basic test config.
async fn setup_test_config() -> Result<(tempfile::TempDir, Arc<RtcBuild>, PathBuf)> {
    let tmpdir = tempfile::tempdir().context("error building tempdir for test")?;
    let cfg = Arc::new(RtcBuild::new_test(tmpdir.path()).await?);
    let asset_file = tmpdir.path().join("test_file");
    tokio::fs::write(&asset_file, b"abc123")
        .await
        .context("error writing test file contents")?;
    Ok((tmpdir, cfg, asset_file))
}

#[tokio::test]
async fn err_new_missing_href() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, _) = setup_test_config().await?;

    // Action.
    let res = CopyFile::new(cfg, Arc::new(tmpdir.into_path()), Default::default(), 0).await;

    // Assert.
    anyhow::ensure!(
        res.is_err(),
        "unexpected success while constructing CopyFile pipeline, expected error on missing \
         `href` attr"
    );

    Ok(())
}

#[tokio::test]
async fn ok_new() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, _) = setup_test_config().await?;
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_HREF.into(), "test_file".into());

    // Action.
    let res = CopyFile::new(cfg, Arc::new(tmpdir.into_path()), attrs, 0).await;

    // Assert.
    anyhow::ensure!(
        res.is_ok(),
        "unexpected failure while constructing CopyFile pipeline"
    );

    Ok(())
}

#[tokio::test]
async fn ok_run_basic_copy() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, asset_file) = setup_test_config().await?;
    let copy_location = cfg.staging_dist.join("test_file");
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_HREF.into(), "test_file".into());
    let cmd = CopyFile::new(cfg, Arc::new(tmpdir.into_path()), attrs, 0)
        .await
        .context("error constructing CopyFile pipeline")?;

    // Action.
    let _out = cmd
        .spawn()
        .await
        .context("unexpected task join error from pipeline")?
        .context("unexpected pipeline error")?;

    // Assert.
    let orig = tokio::fs::read_to_string(asset_file)
        .await
        .context("error reading original file")?;
    let copied = tokio::fs::read_to_string(copy_location)
        .await
        .context("error reading original file")?;
    anyhow::ensure!(
        orig == copied,
        "unexpected content after copy, expected '{}' == '{}'",
        orig,
        copied
    );

    Ok(())
}
