use anyhow::Result;
use clap::{Args, Command, CommandFactory, Subcommand};
use clap_complete::aot::{Generator, Shell, generate};
use std::io::stdout;

use crate::Trunk;

/// Trunk Completion Generater.
#[derive(Clone, Debug, Args)]
#[command(name = "completion")]
pub struct Completion {
    #[command(subcommand)]
    command: ShellCommand,
}

#[derive(Clone, Debug, Subcommand)]
enum ShellCommand {
    /// Generate the trunk completion for bash.
    Bash,
    /// Generate the trunk completion for Elvish.
    Elvish,
    /// Generate the trunk completion for fish.
    Fish,
    /// Generate the trunk completion for PowerShell.
    PowerShell,
    /// Generate the trunk completion for zsh.
    Zsh,
}

async fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    generate(generator, cmd, cmd.get_name().to_string(), &mut stdout());
}

impl Completion {
    #[tracing::instrument(skip(self), err)]
    pub async fn run(self) -> Result<()> {
        let mut cli = Trunk::command();
        match self.command {
            ShellCommand::Bash => print_completions(Shell::Bash, &mut cli).await,
            ShellCommand::Elvish => print_completions(Shell::Elvish, &mut cli).await,
            ShellCommand::Fish => print_completions(Shell::Fish, &mut cli).await,
            ShellCommand::PowerShell => print_completions(Shell::PowerShell, &mut cli).await,
            ShellCommand::Zsh => print_completions(Shell::Zsh, &mut cli).await,
        }
        Ok(())
    }
}
