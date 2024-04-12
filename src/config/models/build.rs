use crate::config::models::BaseUrl;
use clap::Args;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Config options for the build system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
#[command(next_help_heading = "Build")]
pub struct ConfigOptsBuild {
    /// The index HTML file to drive the bundling process [default: index.html]
    pub target: Option<PathBuf>,

    /// Build in release mode [default: false]
    #[arg(long)]
    #[serde(default)]
    pub release: bool,

    /// The output dir for all final assets [default: dist]
    #[arg(short, long)]
    pub dist: Option<PathBuf>,

    /// Run without accessing the network
    #[arg(long)]
    #[serde(default)]
    pub offline: bool,

    /// Require Cargo.lock and cache are up to date
    #[arg(long)]
    #[serde(default)]
    pub frozen: bool,

    /// Require Cargo.lock is up to date
    #[arg(long)]
    #[serde(default)]
    pub locked: bool,

    /// The public URL from which assets are to be served
    #[arg(long)]
    #[serde(default)]
    pub public_url: Option<BaseUrl>,

    /// Don't add a trailing slash to the public URL if it is missing [default: false]
    #[arg(long)]
    #[serde(default)]
    pub public_url_no_trailing_slash_fix: bool,

    /// Build without default features [default: false]
    #[arg(long)]
    #[serde(default)]
    pub no_default_features: bool,

    /// Build with all features [default: false]
    #[arg(long)]
    #[serde(default)]
    pub all_features: bool,

    /// A comma-separated list of features to activate, must not be used with all-features
    /// [default: ""]
    #[arg(long)]
    pub features: Option<String>,

    /// Whether to include hash values in the output file names [default: true]
    #[arg(long)]
    pub filehash: Option<bool>,

    /// Optional pattern for the app loader script [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly load the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub pattern_script: Option<String>,

    /// Whether to inject scripts into your index file. [default: true]
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub inject_scripts: Option<bool>,

    /// Optional pattern for the app preload element [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly preload the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub pattern_preload: Option<String>,

    /// Optional replacement parameters corresponding to the patterns provided in
    /// `pattern_script` and `pattern_preload`.
    ///
    /// When a pattern is being replaced with its corresponding value from this map, if the value
    /// is prefixed with the symbol `@`, then the value is expected to be a file path, and the
    /// pattern will be replaced with the contents of the target file. This allows insertion of
    /// some big JSON state or even HTML files as a part of the `index.html` build.
    ///
    /// Trunk will automatically insert the `base`, `wasm` and `js` key/values into this map. In
    /// order for the app to be loaded properly, the patterns `{base}`, `{wasm}` and `{js}` should
    /// be used in `pattern_script` and `pattern_preload`.
    ///
    /// These values can only be provided via config file.
    #[arg(skip)]
    #[serde(default)]
    pub pattern_params: Option<HashMap<String, String>>,

    /// When desired, set a custom root certificate chain (same format as Cargo's config.toml http.cainfo)
    #[serde(default)]
    #[arg(long)]
    pub root_certificate: Option<String>,

    /// Allows request to ignore certificate validation errors.
    ///
    /// Can be useful when behind a corporate proxy.
    #[serde(default)]
    #[arg(long)]
    pub accept_invalid_certs: Option<bool>,

    /// Allows disabling minification
    #[serde(default)]
    #[arg(long)]
    pub no_minification: bool,

    /// Allows disabling sub-resource integrity (SRI)
    #[serde(default)]
    #[arg(long)]
    pub no_sri: bool,

    /// Ignore error's related to self closing script elements, and instead issue a warning.
    ///
    /// Since this error can cause the HTML output to be truncated, only enable this in case you
    /// are sure it is caused due to a false positive.
    #[serde(default)]
    #[arg(long)]
    pub ignore_script_error: bool,
}
