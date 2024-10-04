#![deny(clippy::expect_used)]
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
mod tls;
mod tools;
mod version;
mod watch;
mod ws;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use common::STARTING;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let cli = Trunk::parse();

    let colored = init_color(&cli);

    tracing_subscriber::registry()
        // Filter spans based on the RUST_LOG env var.
        .with(eval_logging(&cli))
        // Send a copy of all spans to stdout as JSON.
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(colored)
                .with_target(false)
                .with_level(true)
                .compact(),
        )
        // Install this registry as the global tracing registry.
        .try_init()
        .context("error initializing logging")?;

    tracing::info!(
        "{}Starting {} {}",
        STARTING,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    Ok(match cli.run().await {
        Err(err) => {
            tracing::error!("{err}");
            for (n, cause) in err.chain().enumerate().skip(1) {
                tracing::info!("  {n}: {cause}");
            }
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    })
}

fn init_color(cli: &Trunk) -> bool {
    if cli.no_color {
        return false;
    }

    let colored = match cli.color {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => std::io::stdout().is_terminal(),
    };

    #[cfg(windows)]
    if colored {
        if let Err(err) = nu_ansi_term::enable_ansi_support() {
            eprintln!("error enabling ANSI support: {:?}", err);
        }
    }

    #[allow(clippy::let_and_return)]
    colored
}

fn eval_logging(cli: &Trunk) -> tracing_subscriber::EnvFilter {
    // allow overriding everything with RUST_LOG or --log
    if let Some(directives) = &cli.log {
        return tracing_subscriber::EnvFilter::new(directives);
    }

    // allow some sub-commands to be more silent, as their main purpose is to output to the console
    let prefer_silence = cli.prefer_silence();

    let silent = cli.quiet || prefer_silence;

    let directives = match (cli.verbose, silent) {
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
    /// Path to the Trunk config file
    #[arg(long, env = "TRUNK_CONFIG", global(true))]
    pub config: Option<PathBuf>,
    /// Enable verbose logging.
    #[arg(short, long, global(true), action=ArgAction::Count)]
    pub verbose: u8,
    /// Be more quiet, conflicts with --verbose
    #[arg(short, long, global(true), conflicts_with("verbose"))]
    pub quiet: bool,
    /// Provide a RUST_LOG filter, conflicts with --verbose and --quiet
    #[arg(long, global(true), conflicts_with_all(["verbose", "quiet"]), env("RUST_LOG"))]
    pub log: Option<String>,

    /// Skip the version check
    #[arg(long, global(true), env = "TRUNK_SKIP_VERSION_CHECK")]
    pub skip_version_check: bool,

    /// Run without accessing the network
    #[arg(long, global(true), env = "TRUNK_OFFLINE")]
    #[arg(default_missing_value = "true", num_args=0..=1)]
    pub offline: Option<bool>,

    /// Color mode
    #[arg(long, env = "TRUNK_COLOR", global(true), value_enum, conflicts_with = "no_color", default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// Support for `NO_COLOR` environment variable
    #[arg(long, env = "NO_COLOR", global(true))]
    pub no_color: bool,
}

impl Trunk {
    pub fn prefer_silence(&self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match self.action {
            TrunkSubcommands::Config(_) => true,
            TrunkSubcommands::Tools(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default, ValueEnum)]
#[value(rename_all = "lower")]
enum ColorMode {
    /// Enable color when running on a TTY
    #[default]
    Auto,
    /// Always enable color
    Always,
    /// Never enable color
    Never,
}

impl Trunk {
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(self) -> Result<()> {
        version::update_check(self.skip_version_check | self.offline.unwrap_or_default());

        match self.action {
            TrunkSubcommands::Build(inner) => inner.run(self.config).await,
            TrunkSubcommands::Clean(inner) => inner.run(self.config).await,
            TrunkSubcommands::Serve(inner) => inner.run(self.config).await,
            TrunkSubcommands::Watch(inner) => inner.run(self.config).await,
            TrunkSubcommands::Config(inner) => inner.run(self.config).await,
            TrunkSubcommands::Tools(inner) => inner.run(self.config).await,
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
    /// Working with tools
    Tools(cmd::tools::Tools),
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
