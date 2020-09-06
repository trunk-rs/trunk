use std::path::PathBuf;

use anyhow::Result;
use async_std::fs;
use async_process::{Command, Stdio};
use clap::Clap;
use crate::config::Config;

/// Clean output artifacts.
#[derive(Clap)]
#[clap(name="clean")]
pub struct Clean {
    /// The target asset dir. [default: dist]
    #[clap(short, long, parse(from_os_str), env="DIST")]
    dist: Option<PathBuf>,
    /// Optionally perform a `cargo clean`.
    #[clap(short, long, env="CARGO_CLEAN")]
    cargo: bool,
}

impl Clean {
    pub async fn run(self, config: Config) -> Result<()> {
        let conf = CleanConfig::new(self, config);
        let _ = fs::remove_dir_all(&conf.dist).await;
        if conf.cargo {
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

struct CleanConfig {
    dist: PathBuf,
    cargo: bool,
}

impl CleanConfig {
    fn new(clean: Clean, toml_config: Config) -> Self {
        CleanConfig {
            dist: clean.dist.unwrap_or(toml_config.dist),
            cargo: clean.cargo || toml_config.clean.run_cargo_clean
        }
    }
}
