use crate::config::{
    rt::{RtcBuilder, RtcCore},
    Clean, Configuration,
};
use std::ops::Deref;

/// Runtime config for the clean system.
#[derive(Clone, Debug)]
pub struct RtcClean {
    pub core: RtcCore,
    /// Optionally perform a cargo clean.
    pub cargo: bool,
    /// Optionally clean tools.
    pub tools: bool,
}

impl Deref for RtcClean {
    type Target = RtcCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

/// Runtime config options, on a per-run basis.
#[derive(Clone, Debug)]
pub struct CleanOptions {
    pub core: super::CoreOptions,
    pub tools: bool,
}

impl RtcClean {
    pub(crate) fn new(config: Configuration, opts: CleanOptions) -> anyhow::Result<Self> {
        let CleanOptions {
            core: core_opts,
            tools,
        } = opts;

        #[allow(deprecated)]
        let Configuration {
            core: core_config,
            clean:
                Clean {
                    cargo,
                    // We ignore the legacy `dist` field from the configuration for now.
                    // We have a warning in place, and at some point remove this field.
                    dist: _,
                },
            ..
        } = config;

        let core = RtcCore::new(core_config, core_opts)?;

        Ok(Self { core, cargo, tools })
    }
}

impl RtcBuilder for RtcClean {
    type Options = CleanOptions;

    async fn build(configuration: Configuration, options: Self::Options) -> anyhow::Result<Self> {
        Self::new(configuration, options)
    }
}
