#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct NodePackageInformation {
    pub name: String,
    pub version: String,
    #[serde(rename = "dist")]
    pub distribution: NodePackageDistribution,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct NodePackageDistribution {
    /// The legacy, hex encoded SHA-1 digest of the tarball.
    #[serde(default)]
    pub shasum: Option<String>,
    pub tarball: String,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    /// The subresource integrity value of the tarball.
    #[serde(default)]
    pub integrity: Option<String>,
}
