use crate::config::models::ConfigModel;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Config options for the core project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct Core {
    #[serde(default)]
    // align that with cargo's `rust-version`
    #[serde(alias = "trunk-version")]
    #[schemars(with = "VersionReqSchema")]
    pub trunk_version: VersionReq,

    // the dist folder must be relative
    #[serde(default)]
    pub dist: Option<PathBuf>,
}

#[derive(JsonSchema)]
#[schemars(remote = "VersionReq")]
struct VersionReqSchema(#[allow(dead_code)] String);

impl ConfigModel for Core {}
