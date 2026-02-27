#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct NodePackageInformation {
    pub name: String,
    pub version: String,
    #[serde(rename = "dist")]
    pub distribution: NodePackageDistribution,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct NodePackageDistribution {
    pub shasum: String,
    pub tarball: String,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    pub integrity: String,
}
