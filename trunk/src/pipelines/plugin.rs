//! Plugin asset pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_std::task::{JoinHandle, spawn};
use nipper::Document;

use trunk_plugin::{Args, Permissions};

use crate::config::RtcBuild;

use super::{ATTR_HREF, LinkAttrs, TrunkLinkPipelineOutput};

/// An Inline asset pipeline.
pub struct Plugin {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<RtcBuild>,
    /// The directory of the html document.
    html_dir: Arc<PathBuf>,
    /// The arguments for the plugin.
    arguments: Args,
    /// The path to the plugins `*.wasm` file.
    /// This is a temporary solution.
    plugin_file: String,
    /// The permissions the plugin has.
    permissions: Permissions,
}

impl Plugin {
    pub const TYPE_PLUGIN: &'static str = "plugin";

    pub async fn new(cfg: Arc<RtcBuild>, html_dir: Arc<PathBuf>, mut attrs: LinkAttrs, id: usize) -> Result<Self> {
        let plugin_file = attrs
            .remove(ATTR_HREF)
            .context(r#"required attr `href` missing for <link data-trunk rel="plugin" .../> element"#)?;

        let permissions = Permissions::from_link_attrs(&mut attrs);
        let arguments = Args::from_link_attrs(attrs)?
            .with_permissions(permissions);

        Ok(Self {
            id,
            cfg,
            html_dir,
            arguments,
            plugin_file,
            permissions,
        })
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.run())
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<TrunkLinkPipelineOutput> {
        todo!("call the plugin")
    }
}

/// The output of a Inline build pipeline.
pub struct PluginOutput {
    /// The ID of this pipeline.
    pub id: usize,
    /// The permissions of the plugin.
    pub permissions: Permissions,
    /// The output returned from the plugin
    pub output: trunk_plugin::Output,
}

impl PluginOutput {
    pub async fn finalize(self, _dom: &mut Document) -> Result<()> {
        todo!("interpret the plugin output")
    }
}
