mod cmd;

use anyhow::Result;
use colored::*;
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<()> {
    let cli = Trunk::from_args();
    if let Err(err) = cli.run().await {
        println!("{}", err.to_string().red());
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
    pub async fn run(&self) -> Result<()> {
        match &self.action {
            TrunkSubcommands::Build(inner) => inner.run().await,
        }
    }
}

#[derive(StructOpt)]
enum TrunkSubcommands {
    Build(cmd::build::Build),
}
