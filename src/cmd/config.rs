use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::config::ConfigOpts;

/// Trunk config controls.
#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "config")]
pub struct Config {
    #[structopt(subcommand)]
    action: ConfigSubcommands,
}

impl Config {
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        // NOTE WELL: if we ever add additional subcommands, refactor this to match the pattern
        // used in main, which is much more scalable. This is faster to code, and will not force
        // incompatibility when new commands are added.
        match self.action {
            ConfigSubcommands::Show => {
                let cfg = ConfigOpts::full(config).await?;
                println!("{:#?}", cfg);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructOpt)]
enum ConfigSubcommands {
    /// Show Trunk's current config pre-CLI.
    Show,
}
