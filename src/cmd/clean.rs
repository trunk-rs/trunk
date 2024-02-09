use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{ensure, Context, Result};
use clap::Args;
use tokio::process::Command;

use crate::common::remove_dir_all;
use crate::config::{ConfigOpts, ConfigOptsClean};
use crate::tools::cache_dir;
use crate::version::enforce_version;

/// Clean output artifacts.
#[derive(Args)]
#[command(name = "clean")]
pub struct Clean {
    #[command(flatten)]
    pub clean: ConfigOptsClean,
    /// Optionally clean any cached tools used by Trunk
    ///
    /// These tools are cached in a platform dependent "projects" dir. Removing them will cause
    /// them to be downloaded by Trunk next time they are needed.
    #[arg(short, long)]
    pub tools: bool,
}

impl Clean {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_clean(self.clean, config)?;
        enforce_version(&cfg.core)?;

        let _ = remove_dir_all(cfg.dist.clone()).await;
        if cfg.cargo {
            tracing::debug!("cleaning cargo dir");
            let output = Command::new("cargo")
                .arg("clean")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;
            ensure!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if self.tools {
            tracing::debug!("cleaning trunk tools cache dir");
            let path = cache_dir().await.context("error getting cache dir path")?;
            remove_dir_all(path).await?;
        }
        Ok(())
    }
}
