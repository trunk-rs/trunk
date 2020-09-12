mod build;
pub(crate) mod cmd;
mod common;
mod config;
mod serve;
mod watch;

use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<()> {
    let cli = Trunk::from_args();
    if let Err(err) = cli.run().await {
        eprintln!("{}", err.to_string());
    }
    Ok(())
}

/// Build, bundle & ship your Rust WASM application to the web.
#[derive(StructOpt)]
#[structopt(name="trunk")]
struct Trunk {
    #[structopt(subcommand)]
    action: TrunkSubcommands,
    /// Path to the Trunk config file [default: Trunk.toml]
    #[structopt(long, parse(from_os_str), env="TRUNK_CONFIG")]
    pub config: Option<PathBuf>,
}

impl Trunk {
    pub async fn run(self) -> Result<()> {
        match self.action {
            TrunkSubcommands::Build(inner) => inner.run(self.config).await,
            TrunkSubcommands::Clean(inner) => inner.run(self.config).await,
            TrunkSubcommands::Serve(inner) => inner.run(self.config).await,
            TrunkSubcommands::Watch(inner) => inner.run(self.config).await,
        }
    }
}

#[derive(StructOpt)]
enum TrunkSubcommands {
    Build(cmd::build::Build),
    Clean(cmd::clean::Clean),
    Serve(cmd::serve::Serve),
    Watch(cmd::watch::Watch),
}
