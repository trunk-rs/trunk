use crate::config::models::ConfigModel;
use crate::config::Configuration;
use clap::Args;
use schemars::JsonSchema;
use serde::Deserialize;

/// Config options for automatic application downloads.
// **NOTE:** As there are no differences between the persistent configuration and the CLI overrides
// at all, this struct is used for both configuration as well as CLI arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Args, JsonSchema)]
#[command(next_help_heading = "Tools")]
pub struct Tools {
    /// Version of `dart-sass` to use.
    #[serde(default)]
    #[arg(env = "TRUNK_TOOLS_SASS")]
    pub sass: Option<String>,

    /// Version of `wasm-bindgen` to use.
    #[serde(default)]
    #[arg(env = "TRUNK_TOOLS_WASM_BINDGEN")]
    pub wasm_bindgen: Option<String>,

    /// Version of `wasm-opt` to use.
    #[serde(default)]
    #[arg(env = "TRUNK_TOOLS_WASM_OPT")]
    pub wasm_opt: Option<String>,

    /// Version of `tailwindcss-cli` to use.
    #[serde(default)]
    #[arg(env = "TRUNK_TOOLS_TAILWINDCSS")]
    pub tailwindcss: Option<String>,
}

impl Tools {
    pub fn apply_to(self, mut config: Configuration) -> anyhow::Result<Configuration> {
        config.tools.sass = self.sass.or(config.tools.sass);
        config.tools.wasm_bindgen = self.wasm_bindgen.or(config.tools.wasm_bindgen);
        config.tools.wasm_opt = self.wasm_opt.or(config.tools.wasm_opt);
        config.tools.tailwindcss = self.tailwindcss.or(config.tools.tailwindcss);

        Ok(config)
    }
}

impl ConfigModel for Tools {}
