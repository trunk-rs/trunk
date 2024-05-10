use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Minify {
    /// Never minify
    #[default]
    Never,
    /// Minify for release builds
    OnRelease,
    /// Minify for all builds
    Always,
}
