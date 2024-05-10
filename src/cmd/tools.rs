use crate::tools::{self, find_system};
use anyhow::Result;
use clap::{Args, Subcommand};
use console::style;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use strum::IntoEnumIterator;

#[derive(Clone, Debug, Args)]
#[command(name = "tools")]
pub struct Tools {
    #[command(subcommand)]
    action: Option<ToolsSubcommands>,
}

impl Tools {
    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn run(self, _config: Option<PathBuf>) -> Result<()> {
        match self.action {
            None | Some(ToolsSubcommands::Show) => {
                show_tools().await;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Subcommand)]
pub enum ToolsSubcommands {
    /// Show Trunk's tool versions
    Show,
}

async fn show_tools() {
    for app in tools::Application::iter() {
        let (path, version) = find_system(app).await.unzip();
        let path = OrNone(path.map(|p| p.display().to_string()));
        let version = OrNone(version);

        println!("{}", style(app.name()).bold());
        println!("    Installed Version: {version}");
        println!("    Default Version: {}", app.default_version());
        println!(
            "    Download URL: {}",
            OrError(app.url(app.default_version()))
        );

        println!("    Location: {path}");

        println!();
    }
}

struct OrNone<T>(pub Option<T>);

impl<T> Display for OrNone<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(value) => value.fmt(f),
            None => f.write_str("n/a"),
        }
    }
}

struct OrError<T, E>(pub Result<T, E>);

impl<T, E> Display for OrError<T, E>
where
    T: Display,
    E: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Ok(value) => value.fmt(f),
            Err(err) => write!(f, "<Error: {err}>"),
        }
    }
}
