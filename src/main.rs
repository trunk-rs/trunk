mod build;
mod cmd;
mod common;
mod watch;

use anyhow::Result;
use clap::Clap;

#[async_std::main]
async fn main() -> Result<()> {
    let cli = Trunk::parse();
    if let Err(err) = cli.run().await {
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
    pub async fn run(self) -> Result<()> {
        match self.action {
            TrunkSubcommands::Build(inner) => inner.run().await,
            TrunkSubcommands::Clean(inner) => inner.run().await,
            TrunkSubcommands::Serve(inner) => inner.run().await,
            TrunkSubcommands::Watch(inner) => inner.run().await,
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
