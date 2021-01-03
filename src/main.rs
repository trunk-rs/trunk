mod build;
mod cmd;
mod common;
mod config;
mod pipelines;
mod proxy;
mod serve;
mod watch;

use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Trunk::from_args();
    cli.run().await
}

/// Build, bundle & ship your Rust WASM application to the web.
#[derive(StructOpt)]
#[structopt(name = "trunk")]
struct Trunk {
    #[structopt(subcommand)]
    action: TrunkSubcommands,
    /// Path to the Trunk config file [default: Trunk.toml]
    #[structopt(long, parse(from_os_str), env = "TRUNK_CONFIG")]
    pub config: Option<PathBuf>,
}

impl Trunk {
    pub async fn run(self) -> Result<()> {
        match self.action {
            TrunkSubcommands::Build(inner) => inner.run(self.config).await,
            TrunkSubcommands::Clean(inner) => inner.run(self.config).await,
            TrunkSubcommands::Serve(inner) => inner.run(self.config).await,
            TrunkSubcommands::Watch(inner) => inner.run(self.config).await,
            TrunkSubcommands::Config(inner) => inner.run(self.config).await,
        }
    }
}

#[derive(StructOpt)]
enum TrunkSubcommands {
    /// Build the Rust WASM app and all of its assets.
    Build(cmd::build::Build),
    /// Build & watch the Rust WASM app and all of its assets.
    Watch(cmd::watch::Watch),
    /// Build, watch & serve the Rust WASM app and all of its assets.
    Serve(cmd::serve::Serve),
    /// Clean output artifacts.
    Clean(cmd::clean::Clean),
    /// Trunk config controls.
    Config(cmd::config::Config),
}
