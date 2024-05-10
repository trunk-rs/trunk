use crate::config::models::ConfigModel;
use crate::pipelines::PipelineStage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Config options for build system hooks.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Hook {
    /// The stage in the build process to execute this hook.
    pub stage: PipelineStage,
    /// The command to run for this hook.
    pub command: String,
    /// Any arguments to pass to the command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_arguments: Vec<String>,
}

/// Newtype for handling `Vec<Hook>`
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct Hooks(#[serde(default, skip_serializing_if = "Vec::is_empty")] pub Vec<Hook>);

impl ConfigModel for Hooks {}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Mock {
        #[serde(default)]
        hooks: Hooks,
    }

    #[test]
    pub fn test_serde() {
        let value = serde_json::to_value(Mock {
            hooks: Hooks(vec![Hook {
                stage: PipelineStage::PreBuild,
                command: "foo".to_string(),
                command_arguments: vec![],
            }]),
        })
        .expect("must serialize");

        assert_eq!(
            value,
            json!({
                "hooks": [
                    {
                        "stage": "pre_build",
                        "command": "foo",
                    }
                ]
            })
        )
    }

    #[test]
    pub fn test_serde_empty() {
        let value = serde_json::to_value(Mock {
            hooks: Hooks(vec![]),
        })
        .expect("must serialize");

        assert_eq!(
            value,
            json!({
                "hooks": []
            })
        )
    }
}
