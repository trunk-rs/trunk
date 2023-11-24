#![deny(clippy::unwrap_used)]

mod build;
mod cmd;
mod common;
mod config;
mod hooks;
mod pipelines;
mod processing;
mod proxy;
mod serve;
mod tools;
mod watch;
mod ws;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use common::STARTING;
use std::path::PathBuf;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Trunk::parse();

    #[cfg(windows)]
    if let Err(err) = ansi_term::enable_ansi_support() {
        eprintln!("error enabling ANSI support: {:?}", err);
    }

    tracing_subscriber::registry()
        // Filter spans based on the RUST_LOG env var.
        .with(eval_logging(&cli))
        // Send a copy of all spans to stdout as JSON.
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_level(true)
                .compact(),
        )
        // Install this registry as the global tracing registry.
        .try_init()
        .context("error initializing logging")?;

    tracing::info!(
        "{} Starting {} {}",
        STARTING,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    cli.run().await
}

fn eval_logging(cli: &Trunk) -> tracing_subscriber::EnvFilter {
    let directives = match (cli.verbose, cli.quiet) {
        // quiet overrides verbose
        (_, true) => "error,trunk=warn",
        // increase verbosity
        (0, false) => "error,trunk=info",
        (1, false) => "error,trunk=debug",
        (_, false) => "error,trunk=trace",
    };
    tracing_subscriber::EnvFilter::new(directives)
}

/// Build, bundle & ship your Rust WASM application to the web.
#[derive(Parser)]
#[command(about, author, version)]
struct Trunk {
    #[command(subcommand)]
    action: TrunkSubcommands,
    /// Path to the Trunk config file [default: Trunk.toml]
    #[arg(long, env = "TRUNK_CONFIG", global(true))]
    pub config: Option<PathBuf>,
    /// Enable verbose logging.
    #[arg(short, long, global(true), action=ArgAction::Count)]
    pub verbose: u8,
    /// Be more quiet, conflicts with --verbose
    #[arg(short, long, global(true), conflicts_with("verbose"))]
    pub quiet: bool,
}

impl Trunk {
    #[tracing::instrument(level = "trace", skip(self))]
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

#[derive(Subcommand)]
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

#[cfg(test)]
mod tests {
    use crate::Trunk;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Trunk::command().debug_assert();
    }
}
