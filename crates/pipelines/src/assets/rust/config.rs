use std::path::Path;

use trunk_util::Features;

/// A trait that indicates a type can be used as config type for rust app pipeline.
pub trait RustAppConfig {
    /// Returns the public url to be served.
    fn public_url(&self) -> &str;
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;

    /// Returns true if the output file name should contain a file hash.
    fn should_hash(&self) -> bool;

    /// Returns the wasm bindgen version.
    fn wasm_bindgen_version(&self) -> Option<&str>;
    /// Returns the wasm bindgen version.
    fn wasm_opt_version(&self) -> Option<&str>;

    /// Returns true if the final bundle should be optimised.
    fn should_optimize(&self) -> bool;

    /// Returns a number of fallback features.
    fn cargo_features(&self) -> Option<&Features>;

    /// Customise formatter for `<script />` tag.
    fn format_script(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        // Suppress clippy.
        let _ = (script_path, wasm_path);
        None
    }
    /// Customise formatter for the preload tag for WebAssmebly Bundle.
    fn format_preload(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        // Suppress clippy.
        let _ = (script_path, wasm_path);
        None
    }
}
