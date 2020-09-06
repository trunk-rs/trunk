mod build;
mod cmd;
mod common;
mod watch;
mod config;

use anyhow::Result;
use clap::Clap;
use config::{read_config, Config};

#[async_std::main]
async fn main() -> Result<()> {
    let cli = Trunk::parse();
    let config = read_config(None);
    if let Err(err) = cli.run(config).await {
        eprintln!("{}", err.to_string());
    }
    Ok(())
}

/// Build, bundle & ship your Rust WASM application to the web.
#[derive(Clap)]
#[clap(name="trunk")]
struct Trunk {
    #[clap(subcommand)]
    action: TrunkSubcommands
}

impl Trunk {
    pub async fn run(self, config: Config) -> Result<()> {
        match self.action {
            TrunkSubcommands::Build(inner) => inner.run(config).await,
            TrunkSubcommands::Clean(inner) => inner.run(config).await,
            TrunkSubcommands::Serve(inner) => inner.run(config).await,
            TrunkSubcommands::Watch(inner) => inner.run(config).await,
        }
    }
}

#[derive(Clap)]
enum TrunkSubcommands {
    Build(cmd::build::Build),
    Clean(cmd::clean::Clean),
    Serve(cmd::serve::Serve),
    Watch(cmd::watch::Watch),
}
