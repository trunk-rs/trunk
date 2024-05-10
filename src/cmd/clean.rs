use crate::{
    common::remove_dir_all,
    config::{
        self,
        rt::{self, RtcBuilder, RtcClean},
        Configuration,
    },
    tools::cache_dir,
};
use anyhow::{ensure, Context, Result};
use clap::Args;
use std::{path::PathBuf, process::Stdio};
use tokio::process::Command;

/// Clean output artifacts.
#[derive(Clone, Args)]
#[command(name = "clean")]
#[command(next_help_heading = "Clean")]
pub struct Clean {
    /// The output dir for all final assets [default: dist]
    #[arg(short, long, env = "TRUNK_CLEAN_DIST")]
    pub dist: Option<PathBuf>,
    /// Optionally perform a cargo clean [default: false]
    #[arg(long, env = "TRUNK_CLEAN_CARGO")]
    pub cargo: bool,
    /// Optionally clean any cached tools used by Trunk [default: false]
    ///
    /// These tools are cached in a platform-dependent "projects" dir. Removing them will cause
    /// them to be downloaded by Trunk next time they are needed.
    #[arg(short, long, env = "TRUNK_CLEAN_TOOLS")]
    pub tools: bool,
}

impl Clean {
    /// apply CLI overrides to the configuration
    pub fn apply_to(self, mut config: Configuration) -> Result<Configuration> {
        let Self {
            dist,
            cargo,
            tools: _, // used by the CLI only
        } = self;

        if cargo {
            config.clean.cargo = true;
        }

        // the config.clean.dist is handled by migrations
        config.core.dist = dist.or(config.core.dist);

        Ok(config)
    }

    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (cfg, working_directory) = config::load(config).await?;

        let cfg = self.clone().apply_to(cfg)?;

        let cfg = RtcClean::from_config(cfg, working_directory, |_, core| rt::CleanOptions {
            core,
            tools: self.tools,
        })
        .await?;

        cfg.enforce_version()?;

        remove_dir_all(cfg.dist.clone())
            .await
            .context("failed to clean dist directory")?;
        if cfg.cargo {
            tracing::debug!("cleaning cargo dir");
            let output = Command::new("cargo")
                .arg("clean")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;
            ensure!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if cfg.tools {
            tracing::debug!("cleaning trunk tools cache dir");
            let path = cache_dir().await.context("error getting cache dir path")?;
            remove_dir_all(path)
                .await
                .context("failed to clean tools directory")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::models::ConfigModel;

    #[test]
    #[allow(deprecated)]
    fn test_override() {
        let mut config = Configuration {
            clean: config::Clean {
                dist: Some("foo".into()),
                cargo: true,
            },
            ..Default::default()
        };
        config.migrate().expect("must work");

        let result = Clean {
            dist: Some("bar".into()),
            cargo: false,
            tools: true,
        }
        .apply_to(config)
        .expect("must not fail");

        assert_eq!(
            result,
            Configuration {
                core: config::models::Core {
                    trunk_version: Default::default(),
                    // we expect the value from the CLI overrides, but in the core section
                    dist: Some("bar".into())
                },
                clean: {
                    config::models::Clean {
                        // the dist field in the clean section must be empty
                        dist: None,
                        cargo: true,
                    }
                },
                ..Default::default()
            }
        );
    }
}
