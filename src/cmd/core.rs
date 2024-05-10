use crate::config::Configuration;
use clap::Args;
use semver::VersionReq;

/// Core options
#[derive(Clone, Debug, Args)]
#[command(next_help_heading = "Core")]
pub struct Core {
    /// Override the required trunk version
    #[arg(long, env = "TRUNK_REQUIRED_VERSION")]
    pub required_version: Option<VersionReq>,
}

impl Core {
    /// apply CLI overrides to the configuration
    pub fn apply_to(self, mut config: Configuration) -> anyhow::Result<Configuration> {
        let Self { required_version } = self;

        config.core.trunk_version = required_version.unwrap_or(config.core.trunk_version);

        Ok(config)
    }
}
