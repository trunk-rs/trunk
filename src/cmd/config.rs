use crate::config::{self, Configuration};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::{fs::File, io::stdout, path::PathBuf};

/// Trunk config controls.
#[derive(Clone, Debug, Args)]
#[command(name = "config")]
pub struct Config {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Show Trunk's current config pre-CLI.
    Show,
    /// Generate the trunk configuration schema.
    GenerateSchema {
        /// Filename to write the schema to, defaults to `<stdout>`.
        output: Option<PathBuf>,
    },
}

impl Config {
    #[tracing::instrument(skip(self, config), err)]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        match self.command {
            Command::Show => {
                let (cfg, _working_directory) = config::load(config).await?;
                println!("{:#?}", cfg);
            }
            Command::GenerateSchema { output } => {
                let schema = schemars::schema_for!(Configuration);

                match output {
                    Some(file) => {
                        serde_json::to_writer_pretty(File::create(&file)?, &schema)?;
                        println!("Wrote schema to: {}", file.display());
                    }
                    None => {
                        serde_json::to_writer_pretty(stdout().lock(), &schema)?;
                    }
                }
            }
        }
        Ok(())
    }
}
