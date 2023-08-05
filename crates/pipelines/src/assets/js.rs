//! JS asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::{ok, BoxFuture};
use futures_util::stream::BoxStream;
use futures_util::FutureExt;
use nipper::Document;
use tokio::task::JoinHandle;

use super::{Asset, Output};
// use super::{TrunkAssetPipelineOutput, ATTR_SRC};
use crate::asset_file::AssetFile;
use crate::util::{Attrs, ErrorReason, Result, ResultExt, ATTR_SRC};

/// A trait that indicates a type can be used as config type for js pipeline.
pub trait JsConfig {
    /// Returns the public url to be served.
    fn public_url(&self) -> &str;
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;

    /// Returns true if the output file name should contain a file hash.
    fn should_hash(&self) -> bool;
}

/// A JS asset pipeline.
pub struct Js<C> {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The asset file being processed.
    asset: AssetFile,
    /// The attributes to be placed on the output script tag.
    attrs: Attrs,
}

impl<C> Js<C>
where
    C: JsConfig,
{
    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let src_attr = attrs
            .get(ATTR_SRC)
            .reason(ErrorReason::PipelineScriptSrcNotFound)?;
        let mut path = PathBuf::new();
        path.extend(src_attr.split('/'));
        let asset = AssetFile::new(&html_dir, path).await?;
        // Remove src and data-trunk from attributes.
        let attrs = attrs
            .into_iter()
            .filter(|(x, _)| *x != "src" && !x.starts_with("data-trunk"))
            .collect();
        Ok(Self {
            id,
            cfg,
            asset,
            attrs,
        })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(&self) -> Result<JsOutput<C>> {
        let rel_path = crate::util::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "copying & hashing js");
        let file = self
            .asset
            .copy(self.cfg.output_dir(), self.cfg.should_hash())
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing js");
        let attrs = Self::attrs_to_string(&self.attrs);
        Ok(JsOutput {
            cfg: self.cfg.clone(),
            id: self.id,
            file,
            attrs,
        })
    }

    /// Convert attributes to a string, to be used in JsOutput.
    fn attrs_to_string(attrs: &Attrs) -> String {
        attrs
            .iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl<C> Asset for Js<C>
where
    C: 'static + JsConfig + Send + Sync,
{
    type Output = JsOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;
    type RunOnceFuture<'a> = BoxFuture<'a, Result<Self::Output>>;

    fn run_once(&self, input: super::AssetInput) -> Self::RunOnceFuture<'_> {
        self.run().boxed()
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }

    /// Spawn the pipeline for this asset type.
    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<JsOutput<C>>> {
        tokio::spawn(async move { self.run().await })
    }
}

/// The output of a JS build pipeline.
pub struct JsOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
    /// The ID of this pipeline.
    pub id: usize,
    /// Name of the finalized output file.
    pub file: String,
    /// The attributes to be added to the script tag.
    pub attrs: String,
}

impl<C> Output for JsOutput<C>
where
    C: JsConfig + Send + Sync,
{
    fn finalize<'life0, 'async_trait>(
        self,
        dom: &'life0 mut Document,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<()>> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        dom.select(&crate::util::trunk_script_id_selector(self.id))
            .replace_with_html(format!(
                r#"<script {attrs} src="{base}{file}"/>"#,
                attrs = self.attrs,
                base = &self.cfg.public_url(),
                file = self.file
            ));
        ok(()).boxed()
    }
}
