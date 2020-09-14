use std::path::PathBuf;

use anyhow::Result;
use async_std::fs;
use async_process::{Command, Stdio};
use structopt::StructOpt;

use crate::config::{ConfigOpts, ConfigOptsClean};

/// Clean output artifacts.
#[derive(StructOpt)]
#[structopt(name="clean")]
pub struct Clean {
    #[structopt(flatten)]
    pub clean: ConfigOptsClean,
}

impl Clean {
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_clean(self.clean, config).await?;
        let _ = fs::remove_dir_all(&cfg.dist).await;
        if cfg.cargo {
            let output = Command::new("cargo")
                .arg("clean")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Ok(())
    }
}
