mod build;
mod cmd;
mod common;
mod watch;

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

#[derive(StructOpt)]
enum TrunkSubcommands {
    Build(cmd::build::Build),
    Clean(cmd::clean::Clean),
    Serve(cmd::serve::Serve),
    Watch(cmd::watch::Watch),
}
