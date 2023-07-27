use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::util::ATTR_HREF;
use crate::{CopyDir, CopyDirConfig, Pipeline};

struct CopyDirTestConfig {
    output_dir: PathBuf,
}

impl CopyDirConfig for CopyDirTestConfig {
    fn output_dir(&self) -> &std::path::Path {
        &self.output_dir
    }
}

/// A fixture for setting up basic test config.
async fn setup_test_config() -> Result<(tempfile::TempDir, Arc<CopyDirTestConfig>, PathBuf)> {
    let tmpdir = tempfile::tempdir().context("error building tempdir for test")?;
    let cfg = Arc::new(CopyDirTestConfig {
        output_dir: tmpdir.path().to_owned(),
    });
    let asset_dir = tmpdir.path().join("test_dir");
    tokio::fs::create_dir(&asset_dir)
        .await
        .context("error creating test dir")?;
    let asset_file = asset_dir.join("test_file");
    tokio::fs::write(&asset_file, b"abc123")
        .await
        .context("error writing test file contents")?;
    Ok((tmpdir, cfg, asset_dir))
}

#[tokio::test]
async fn err_new_missing_href() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, _) = setup_test_config().await?;

    // Action.
    let res = CopyDir::new(cfg, Arc::new(tmpdir.into_path()), Default::default(), 0).await;

    // Assert.
    anyhow::ensure!(
        res.is_err(),
        "unexpected success while constructing CopyDir pipeline, expected error on missing `href` \
         attr"
    );

    Ok(())
}

#[tokio::test]
async fn ok_new() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, _) = setup_test_config().await?;
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_HREF.into(), "test_dir".into());

    // Action.
    let res = CopyDir::new(cfg, Arc::new(tmpdir.into_path()), attrs, 0).await;

    // Assert.
    anyhow::ensure!(
        res.is_ok(),
        "unexpected failure while constructing CopyDir pipeline"
    );

    Ok(())
}

#[tokio::test]
async fn ok_run_basic_copy() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, asset_dir) = setup_test_config().await?;
    let copy_location_dir = cfg.output_dir().join("test_dir");
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_HREF.into(), "test_dir".into());
    let cmd = CopyDir::new(cfg, Arc::new(tmpdir.into_path()), attrs, 0)
        .await
        .context("error constructing CopyDir pipeline")?;

    // Action.
    let _out = cmd
        .spawn()
        .await
        .context("unexpected task join error from pipeline")?
        .context("unexpected pipeline error")?;

    // Assert.
    let orig_file = tokio::fs::read_to_string(asset_dir.join("test_file"))
        .await
        .context("error reading original file")?;
    let copied_file = tokio::fs::read_to_string(copy_location_dir.join("test_file"))
        .await
        .context("error reading copied file")?;
    anyhow::ensure!(
        copy_location_dir.is_dir(),
        "expected '{}' to be a directory",
        copy_location_dir.display(),
    );
    anyhow::ensure!(
        orig_file == copied_file,
        "unexpected content after copy, expected '{}' == '{}'",
        orig_file,
        copied_file
    );

    Ok(())
}

#[tokio::test]
async fn ok_run_target_path_copy() -> Result<()> {
    // Assemble.
    let (tmpdir, cfg, asset_dir) = setup_test_config().await?;
    let copy_location_dir = cfg.output_dir().join("not-test_dir");
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_HREF.into(), "test_dir".into());
    attrs.insert("data-target-path".into(), "not-test_dir".into());
    let cmd = CopyDir::new(cfg, Arc::new(tmpdir.into_path()), attrs, 0)
        .await
        .context("error constructing CopyDir pipeline")?;

    // Action.
    let _out = cmd
        .spawn()
        .await
        .context("unexpected task join error from pipeline")?
        .context("unexpected pipeline error")?;

    // Assert.
    let orig_file = tokio::fs::read_to_string(asset_dir.join("test_file"))
        .await
        .context("error reading original file")?;
    let copied_file = tokio::fs::read_to_string(copy_location_dir.join("test_file"))
        .await
        .context("error reading copied file")?;
    anyhow::ensure!(
        copy_location_dir.is_dir(),
        "expected '{}' to be a directory",
        copy_location_dir.display(),
    );
    anyhow::ensure!(
        orig_file == copied_file,
        "unexpected content after copy, expected '{}' == '{}'",
        orig_file,
        copied_file
    );

    Ok(())
}
