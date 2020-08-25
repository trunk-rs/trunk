use std::path::PathBuf;

use anyhow::Result;
use async_std::fs;
use async_process::{Command, Stdio};
use structopt::StructOpt;

/// Clean output artifacts.
#[derive(StructOpt)]
#[structopt(name="clean")]
pub struct Clean {
    /// The target asset dir.
    #[structopt(short, long, default_value="dist", parse(from_os_str))]
    dist: PathBuf,
    /// Optionally perform a `cargo clean`.
    #[structopt(short, long)]
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
