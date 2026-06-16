use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// An algorithm used to pre-compress build assets into sidecar files.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize, JsonSchema, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompressionAlgorithm {
    /// gzip (RFC 1952), emitted as a `.gz` sidecar.
    Gzip,
    /// Brotli, emitted as a `.br` sidecar.
    Brotli,
}

impl CompressionAlgorithm {
    /// The file extension (without leading dot) used for the sidecar file.
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Gzip => "gz",
            Self::Brotli => "br",
        }
    }
}

/// How much effort to spend compressing assets, trading speed for size.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    Serialize,
    JsonSchema,
    Display,
    EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompressionLevel {
    /// Fastest, largest output.
    Low,
    /// Balanced speed and size (the default).
    #[default]
    Medium,
    /// Slowest, smallest output.
    High,
}
