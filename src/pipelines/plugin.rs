use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_std::task::{spawn, JoinHandle};
use nipper::Document;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;

use super::{AssetFile, LinkAttrs, TrunkLinkPipelineOutput, ATTR_HREF, ATTR_TYPE};

const PLUGIN_ATTR: &str = "data-plugin-name";

/// An Inline asset pipeline.
pub struct Plugin {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    plugin_name: String,
}

impl Plugin {
    pub const TYPE_PLUGIN: &'static str = "plugin";

    pub async fn new(html_dir: Arc<PathBuf>, attrs: LinkAttrs, id: usize) -> Result<Self> {
        let plugin_name = attrs
            .get(PLUGIN_ATTR)
            .with_context(|| format!("plugin pipelines require attr {}", PLUGIN_ATTR))?
            .clone();

        println!("in plugin pipeline");
        for (key, val) in attrs.iter() {
            println!("key: {}; val: {}", key, val);
        }

        Ok(Self { id, plugin_name })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkLinkPipelineOutput> {
        let client = reqwest::Client::new();
        let query =
            r#"query QueryForPackage($pkg: String!) { getPackageVersion(name:$pkg) { package { name } version modules { name publicUrl } } }"#;
        let res = client
            .post("https://registry.wapm.io/graphql")
            .json(&json! {{
                "query": query,
                "variables": {
                    "pkg": self.plugin_name,
                }
            }})
            .send()
            .await?;
        let body = res.text().await.context("error extracting body")?;
        println!("response: {:?}", body);
        anyhow::bail!("hacking");
    }
}

pub struct PluginOutput;

impl PluginOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        // let html = match self.content_type {
        //     ContentType::Html => self.content,
        //     ContentType::Css => format!(r#"<style type="text/css">{}</style>"#, self.content),
        //     ContentType::Js => format!(r#"<script>{}</script>"#, self.content),
        // };

        // dom.select(&super::trunk_id_selector(self.id)).replace_with_html(html);
        Ok(())
    }
}

// #[derive(Deserialize)]
// struct GraphQLData<T: DeserializeOwned> {
//     pub data: T,
// }

// // {
// //     "getPackageVersion": {
// //       "package": {
// //         "name": "_/python"
// //       },
// //       "version": "0.1.0",
// //       "modules": [
// //         {
// //           "name": "python",
// //           "publicUrl": "https://registry-cdn.wapm.io/contents/_/python/0.1.0/bin/python.wasm"
// //         }
// //       ]
// //     }
// // }

// #[derive(Deserialize)]
// struct WapmResponse {
//     #[serde(rename = "getPackageVersion")]
//     pub get_package_version: (),
// }
