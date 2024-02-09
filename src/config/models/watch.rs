use crate::config::models::ConfigDuration;
use clap::Args;
use serde::Deserialize;
use std::path::PathBuf;

/// Config options for the watch system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
#[command(next_help_heading = "Watch")]
pub struct ConfigOptsWatch {
    /// Watch specific file(s) or folder(s) [default: build target parent folder]
    #[arg(short, long, value_name = "path")]
    pub watch: Option<Vec<PathBuf>>,
    /// Paths to ignore [default: []]
    #[arg(short, long, value_name = "path")]
    pub ignore: Option<Vec<PathBuf>>,
    /// Using polling mode for detecting changes
    #[arg(long)]
    #[serde(default)]
    pub poll: bool,
    /// The polling interval, when polling is enabled
    #[arg(long)]
    #[serde(default)]
    pub poll_interval: Option<ConfigDuration>,
    /// Allow enabling a cooldown, discarding all change events during the build [default: false]
    #[arg(long)]
    #[serde(default)]
    pub enable_cooldown: bool,
}
