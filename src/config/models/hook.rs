use crate::pipelines::PipelineStage;
use serde::Deserialize;

/// Config options for build system hooks.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigOptsHook {
    /// The stage in the build process to execute this hook.
    pub stage: PipelineStage,
    /// The command to run for this hook.
    pub command: String,
    /// Any arguments to pass to the command.
    #[serde(default)]
    pub command_arguments: Vec<String>,
}
