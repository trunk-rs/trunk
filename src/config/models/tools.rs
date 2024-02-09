use serde::Deserialize;

/// Config options for automatic application downloads.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigOptsTools {
    /// Version of `dart-sass` to use.
    pub sass: Option<String>,
    /// Version of `wasm-bindgen` to use.
    pub wasm_bindgen: Option<String>,
    /// Version of `wasm-opt` to use.
    pub wasm_opt: Option<String>,
    /// Version of `tailwindcss-cli` to use.
    pub tailwindcss: Option<String>,
}
