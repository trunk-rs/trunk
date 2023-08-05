//! Inline asset pipeline.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::ok;
use futures_util::stream::BoxStream;
use futures_util::FutureExt;
use nipper::Document;
use tokio::task::JoinHandle;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{
    trunk_id_selector, Attrs, Error, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_TYPE,
};

/// An Inline asset pipeline.
pub struct Inline {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// The asset file being processed.
    asset: AssetFile,
    /// The type of the asset file that determines how the content of the file
    /// is inserted into `index.html`.
    content_type: ContentType,
}

impl Inline {
    pub const TYPE_INLINE: &'static str = "inline";

    pub async fn new(html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        let href_attr =
            attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: "inline".into(),
                })?;

        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));

        let asset = AssetFile::new(&html_dir, path).await?;
        let content_type =
            ContentType::from_attr_or_ext(attrs.get(ATTR_TYPE), asset.ext.as_deref())?;

        Ok(Self {
            id,
            asset,
            content_type,
        })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(&self) -> Result<InlineOutput> {
        let rel_path = crate::util::strip_prefix(&self.asset.path);
        tracing::info!(path = ?rel_path, "reading file content");
        let content = self.asset.read_to_string().await?;
        tracing::info!(path = ?rel_path, "finished reading file content");

        Ok(InlineOutput {
            id: self.id,
            content,
            content_type: self.content_type.clone(),
        })
    }
}

#[async_trait]
impl Asset for Inline {
    type Output = InlineOutput;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn run_once(&self, input: super::AssetInput) -> Result<Self::Output> {
        self.run().await
    }

    fn outputs(self) -> Self::OutputStream {
        todo!()
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<InlineOutput>> {
        tokio::spawn(async move { self.run().await })
    }
}

/// The content type of a inlined file.
#[derive(Debug, Clone)]
pub enum ContentType {
    /// Html is just pasted into `index.html` as is.
    Html,
    /// Svg is just pasted into `index.html` as is.
    Svg,
    /// CSS is wrapped into `style` tags.
    Css,
    /// JS is wrapped into `script` tags.
    Js,
}

impl ContentType {
    /// Either tries to parse the provided attribute to a ContentType
    /// or tries to infer the ContentType from the AssetFile extension.
    fn from_attr_or_ext(attr: Option<impl AsRef<str>>, ext: Option<&str>) -> Result<Self> {
        match attr {
            Some(attr) => Self::from_str(attr.as_ref()),
            None => match ext {
                Some(ext) => Self::from_str(ext),
                None => Err(ErrorReason::PipelineLinkInlineTypeNotFound.into_error()),
            },
        }
    }
}

impl FromStr for ContentType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "html" => Ok(Self::Html),
            "css" => Ok(Self::Css),
            "js" => Ok(Self::Js),
            "svg" => Ok(Self::Svg),
            s => Err(ErrorReason::PipelineLinkInlineTypeNotSupported {
                type_value: s.to_owned().into(),
            }
            .into_error()),
        }
    }
}

/// The output of a Inline build pipeline.
pub struct InlineOutput {
    /// The ID of this pipeline.
    pub id: usize,
    /// The content of the target file.
    pub content: String,
    /// The content type of the target file.
    pub content_type: ContentType,
}

impl Output for InlineOutput {
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
        let html = match self.content_type {
            ContentType::Html | ContentType::Svg => self.content,
            ContentType::Css => format!(r#"<style type="text/css">{}</style>"#, self.content),
            ContentType::Js => format!(r#"<script>{}</script>"#, self.content),
        };

        dom.select(&trunk_id_selector(self.id))
            .replace_with_html(html);
        ok(()).boxed()
    }
}
