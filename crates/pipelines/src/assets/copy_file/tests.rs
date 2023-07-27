use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::assets::{Asset, CopyFile, CopyFileConfig};
// use crate::config::RtcBuild;
use crate::util::ATTR_HREF;

struct CopyFileTestConfig {
    output_dir: PathBuf,
}

impl CopyFileConfig for CopyFileTestConfig {
    fn output_dir(&self) -> &std::path::Path {
        &self.output_dir
    }
}

/// A fixture for setting up basic test config.
async fn setup_test_config() -> Result<(tempfile::TempDir, Arc<CopyFileTestConfig>, PathBuf)> {
    let tmpdir = tempfile::tempdir().context("error building tempdir for test")?;
    let cfg = Arc::new(CopyFileTestConfig {
        output_dir: tmpdir.path().to_owned(),
    });
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
    let copy_location = cfg.output_dir().join("test_file");
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
