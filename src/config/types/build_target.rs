use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BuildTarget {
    /// wasm32-unknown-unknown
    #[default]
    Wasm32UnknownUnknown,
    /// Don't build anything
    None,
}
