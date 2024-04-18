#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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
