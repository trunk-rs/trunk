//! JS asset pipeline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::BoxStream;
use nipper::Document;
use trunk_util::AssetInput;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{Attrs, ErrorReason, Result, ResultExt, ATTR_SRC};

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,

    /// The asset file being processed.
    file: AssetFile,
}

impl Input {
    async fn try_from(input: AssetInput) -> Result<Self> {
        if input.tag_name.to_lowercase() != "script" {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        // Build the path to the target asset.
        let src_attr = input
            .attrs
            .get(ATTR_SRC)
            .reason(ErrorReason::PipelineScriptSrcNotFound)?;
        let mut path = PathBuf::new();
        path.extend(src_attr.split('/'));
        let asset = AssetFile::new(&input.manifest_dir, path).await?;

        let input = Input {
            asset_input: input,
            file: asset,
        };

        Ok(input)
    }
}

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
    /// Runtime build config.
    cfg: Arc<C>,
    inputs: Vec<Input>,
}

impl<C> Js<C>
where
    C: JsConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            inputs: Vec::new(),
        }
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run_with_input(&self, input: Input) -> Result<JsOutput<C>> {
        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "copying & hashing js");
        let file = input
            .file
            .copy(self.cfg.output_dir(), self.cfg.should_hash())
            .await?;
        tracing::info!(path = ?rel_path, "finished copying & hashing js");
        // Remove src and data-trunk from attributes.
        let attrs = input
            .asset_input
            .attrs
            .into_iter()
            .filter(|(x, _)| *x != "src" && !x.starts_with("data-trunk"))
            .collect::<HashMap<_, _>>();

        let attrs = Self::attrs_to_string(&attrs);
        Ok(JsOutput {
            cfg: self.cfg.clone(),
            id: input.asset_input.id,
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

#[async_trait]
impl<C> Asset for Js<C>
where
    C: 'static + JsConfig + Send + Sync,
{
    type Output = JsOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        self.run_with_input(input).await
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
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

#[async_trait(?Send)]
impl<C> Output for JsOutput<C>
where
    C: JsConfig + Send + Sync,
{
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        dom.select(&crate::util::trunk_script_id_selector(self.id))
            .replace_with_html(format!(
                r#"<script {attrs} src="{base}{file}"/>"#,
                attrs = self.attrs,
                base = &self.cfg.public_url(),
                file = self.file
            ));
        Ok(())
    }
}
