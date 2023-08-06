//! Inline asset pipeline.

use std::path::PathBuf;
use std::str::FromStr;

use async_trait::async_trait;
use futures_util::stream::{self, BoxStream};
use futures_util::StreamExt;
use nipper::Document;
use trunk_util::AssetInput;

use super::{Asset, Output};
use crate::asset_file::AssetFile;
use crate::util::{
    trunk_id_selector, Error, ErrorReason, Result, ResultExt, ATTR_HREF, ATTR_REL, ATTR_TYPE,
};

static TYPE_INLINE: &str = "inline";

#[derive(Debug)]
struct Input {
    asset_input: AssetInput,

    /// The asset file being processed.
    file: AssetFile,

    /// The type of the asset file that determines how the content of the file
    /// is inserted into `index.html`.
    content_type: ContentType,
}

impl Input {
    async fn try_from(input: AssetInput) -> Result<Self> {
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_INLINE) {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        let href_attr =
            input
                .attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: "inline".into(),
                })?;

        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));

        let asset = AssetFile::new(&input.manifest_dir, path).await?;
        let content_type =
            ContentType::from_attr_or_ext(input.attrs.get(ATTR_TYPE), asset.ext.as_deref())?;

        let input = Input {
            asset_input: input,
            file: asset,
            content_type,
        };

        Ok(input)
    }
}

/// An Inline asset pipeline.
#[derive(Default)]
pub struct Inline {
    inputs: Vec<Input>,
}

impl Inline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace")]
    async fn run_with_input(input: Input) -> Result<InlineOutput> {
        let rel_path = crate::util::strip_prefix(&input.file.path);
        tracing::info!(path = ?rel_path, "reading file content");
        let content = input.file.read_to_string().await?;
        tracing::info!(path = ?rel_path, "finished reading file content");

        Ok(InlineOutput {
            id: input.asset_input.id,
            content,
            content_type: input.content_type,
        })
    }
}

#[async_trait]
impl Asset for Inline {
    type Output = InlineOutput;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(input).await?;
        Self::run_with_input(input).await
    }

    fn outputs(self) -> Self::OutputStream {
        let Self { inputs } = self;

        stream::iter(inputs.into_iter())
            .then(move |input| tokio::spawn(async move { Self::run_with_input(input).await }))
            .map(|m| match m.reason(ErrorReason::TokioTaskFailed) {
                Ok(Ok(m)) => Ok(m),
                Ok(Err(e)) | Err(e) => Err(e),
            })
            .boxed()
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

#[async_trait(?Send)]
impl Output for InlineOutput {
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        let html = match self.content_type {
            ContentType::Html | ContentType::Svg => self.content,
            ContentType::Css => format!(r#"<style type="text/css">{}</style>"#, self.content),
            ContentType::Js => format!(r#"<script>{}</script>"#, self.content),
        };

        dom.select(&trunk_id_selector(self.id))
            .replace_with_html(html);
        Ok(())
    }
}
