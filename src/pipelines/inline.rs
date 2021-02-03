//! Inline asset pipeline.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_std::task::{spawn, JoinHandle};
use indicatif::ProgressBar;
use nipper::{Document, Selection};

use super::{AssetFile, TrunkLinkPipelineOutput, ATTR_HREF, ATTR_TYPE};

/// An Inline asset pipeline.
pub struct Inline {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// The progress bar to use for this pipeline.
    progress: ProgressBar,
    /// The asset file being processed.
    asset: AssetFile,
    /// The type of the asset file that determines how the content of the file
    /// is inserted into `index.html`.
    content_type: ContentType,
}

impl Inline {
    pub const TYPE_INLINE: &'static str = "inline";

    pub async fn new(progress: ProgressBar, html_dir: Arc<PathBuf>, el: Selection<'_>, id: usize) -> Result<Self> {
        let href_attr = el
            .attr(ATTR_HREF)
            .ok_or_else(|| anyhow!("required attr `href` missing for <link data-trunk .../> element: {}", el.html()))?;

        let mut path = PathBuf::new();
        path.extend(href_attr.as_ref().split('/'));

        let asset = AssetFile::new(&html_dir, path).await?;
        let content_type = ContentType::from_attr_or_ext(el.attr(ATTR_TYPE), &asset.ext)?;

        Ok(Self {
            id,
            progress,
            asset,
            content_type,
        })
    }

    /// Spawn the pipeline for this asset type.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(async move {
            self.progress.set_message("reading file content");
            let content = self.asset.read_to_string().await?;
            self.progress.set_message("finished reading file content");
            Ok(TrunkLinkPipelineOutput::Inline(InlineOutput {
                id: self.id,
                content,
                content_type: self.content_type,
            }))
        })
    }
}

/// The content type of a inlined file.
pub enum ContentType {
    /// Html is just pasted into `index.html` as is.
    Html,
    /// CSS is wrapped into `style` tags.
    CSS,
    /// JS is wrapped into `script` tags.
    JS,
}

impl ContentType {
    /// Either tries to parse the provided attribute to a ContentType
    /// or tries to infer the ContentType from the AssetFile extension.
    fn from_attr_or_ext(attr: Option<impl AsRef<str>>, ext: &str) -> Result<Self> {
        match attr {
            Some(attr) => Self::from_str(attr.as_ref()),
            None => Self::from_str(ext),
        }
    }
}

impl FromStr for ContentType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "html" => Ok(Self::Html),
            "css" => Ok(Self::CSS),
            "js" => Ok(Self::JS),
            s => bail!(
                r#"unknown `type="{}"` value for <link data-trunk rel="inline" .../> attr; please ensure the value is lowercase and is a supported content type"#,
                s
            ),
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

impl InlineOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let html = match self.content_type {
            ContentType::Html => self.content,
            ContentType::CSS => format!("<style>{}</style>", self.content),
            ContentType::JS => format!("<script>{}</script>", self.content),
        };

        dom.select(&super::trunk_id_selector(self.id)).replace_with_html(html);
        Ok(())
    }
}
