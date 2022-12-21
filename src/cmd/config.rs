use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::config::ConfigOpts;

/// Trunk config controls.
#[derive(Clone, Debug, Args)]
#[command(name = "config")]
pub struct Config {
    #[command(subcommand)]
    action: ConfigSubcommands,
}

impl Config {
    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        // NOTE WELL: if we ever add additional subcommands, refactor this to match the pattern
        // used in main, which is much more scalable. This is faster to code, and will not force
        // incompatibility when new commands are added.
        match self.action {
            ConfigSubcommands::Show => {
                let cfg = ConfigOpts::full(config)?;
                println!("{:#?}", cfg);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Subcommand)]
enum ConfigSubcommands {
    /// Show Trunk's current config pre-CLI.
    Show,
}
