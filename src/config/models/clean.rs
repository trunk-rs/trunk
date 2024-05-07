use clap::Args;
use serde::Deserialize;
use std::path::PathBuf;

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
#[command(next_help_heading = "Clean")]
pub struct ConfigOptsClean {
    /// The output dir for all final assets [default: dist]
    #[arg(short, long)]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean [default: false]
    #[arg(long)]
    #[serde(default)]
    pub cargo: bool,
}
