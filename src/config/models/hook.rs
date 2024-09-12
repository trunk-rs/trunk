use crate::pipelines::PipelineStage;
use serde::Deserialize;

/// Config options for build system hooks.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigOptsHook {
    /// The stage in the build process to execute this hook.
    pub stage: PipelineStage,
    /// The command to run for this hook.
    command: String,
    /// Any arguments to pass to the command.
    #[serde(default)]
    command_arguments: Vec<String>,
    /// Overrides
    #[serde(default, flatten)]
    overrides: ConfigOptsHookOverrride,
}

impl ConfigOptsHook {
    pub fn command(&self) -> &String {
        if cfg!(target_os = "windows") {
            if let Some(cfg) = self.overrides.windows.as_ref() {
                return &cfg.command;
            }
        } else if cfg!(target_os = "macos") {
            if let Some(cfg) = self.overrides.macos.as_ref() {
                return &cfg.command;
            }
        } else if cfg!(target_os = "linux") {
            if let Some(cfg) = self.overrides.linux.as_ref() {
                return &cfg.command;
            }
        }

        &self.command
    }

    pub fn command_arguments(&self) -> &Vec<String> {
        if cfg!(target_os = "windows") {
            if let Some(cfg) = self.overrides.windows.as_ref() {
                return &cfg.command_arguments;
            }
        } else if cfg!(target_os = "macos") {
            if let Some(cfg) = self.overrides.macos.as_ref() {
                return &cfg.command_arguments;
            }
        } else if cfg!(target_os = "linux") {
            if let Some(cfg) = self.overrides.linux.as_ref() {
                return &cfg.command_arguments;
            }
        }

        &self.command_arguments
    }
}

/// Config options for build system hooks.
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ConfigOptsHookOverrride {
    pub windows: Option<ConfigOptsHookOs>,
    pub macos: Option<ConfigOptsHookOs>,
    pub linux: Option<ConfigOptsHookOs>,
}

/// OS-specific overrides.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigOptsHookOs {
    /// The command to run for this hook.
    pub command: String,
    /// Any arguments to pass to the command.
    #[serde(default)]
    pub command_arguments: Vec<String>,
}
