use std::path::PathBuf;

use anyhow::Result;
use async_std::fs;
use async_process::{Command, Stdio};
use clap::Clap;

/// Clean output artifacts.
#[derive(Clap)]
#[clap(name="clean")]
pub struct Clean {
    /// The target asset dir.
    #[clap(short, long, default_value="dist", parse(from_os_str), env="DIST")]
    dist: PathBuf,
    /// Optionally perform a `cargo clean`.
    #[clap(short, long, env="CARGO_CLEAN")]
    cargo: bool,
}

impl Clean {
    pub async fn run(self) -> Result<()> {
        let _ = fs::remove_dir_all(&self.dist).await;
        if self.cargo {
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
