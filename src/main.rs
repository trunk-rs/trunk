#![deny(clippy::unwrap_used)]

mod build;
mod cmd;
mod common;
mod config;
mod hooks;
mod pipelines;
mod proxy;
mod serve;
mod tools;
mod watch;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
        .with(tracing_subscriber::EnvFilter::new(if cli.v {
            "error,trunk=debug"
        } else {
            "error,trunk=info"
        }))
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

    cli.run().await
}

/// Build, bundle & ship your Rust WASM application to the web.
#[derive(Parser)]
#[clap(about, author, version, name = "trunk")]
struct Trunk {
    #[clap(subcommand)]
    action: TrunkSubcommands,
    /// Path to the Trunk config file [default: Trunk.toml]
    #[clap(long, parse(from_os_str), env = "TRUNK_CONFIG")]
    pub config: Option<PathBuf>,
    /// Enable verbose logging.
    #[clap(short)]
    pub v: bool,
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
