use anyhow::{Result, bail};
use lol_html::{ElementContentHandlers, HtmlRewriter, Selector, Settings, html_content::Element};
use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct DocumentOptions {
    /// Ignore self-closing script warnings
    pub allow_self_closing_script: bool,
}

/// A wrapper for Html modifications, and rewrites.
#[derive(Debug)]
pub struct Document {
    html: Vec<u8>,
    asset_fragments: Option<Vec<AssetFragment>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AssetFragment {
    pub(crate) selector: String,
    pub(crate) html: String,
}

impl AsRef<[u8]> for Document {
    fn as_ref(&self) -> &[u8] {
        &self.html
    }
}

impl Document {
    /// Create a new document
    ///
    /// This will fail if a non-valid HTML is provided or a self-closing script element is found
    /// and the self-closing script element is not allowed in the options.
    pub fn new(data: impl Into<Vec<u8>>, options: DocumentOptions) -> Result<Self> {
        let doc = Self {
            html: data.into(),
            asset_fragments: None,
        };

        // Check for self-closed script tags such as "<script.../>"
        doc.select("script", |el| {
            if el.is_self_closing() {
                if options.allow_self_closing_script {
                    tracing::warn!("Self-closing script tag found (allowed by configuration)");
                }
                else {
                     bail!(
                        r#"Self-closing script tag found.

Replace the self-closing script tag ("<script .../>") with a normally closed one such as "<script ...></script>".
For more information, please take a look at https://github.com/trunk-rs/trunk/discussions/771."

In case this is a false positive, the "--allow-self-closing-script" flag can be used to issue a warning instead."#
                    )
                }
            }
            Ok(())
        })?;

        Ok(doc)
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.html
    }

    pub(crate) fn start_asset_capture(&mut self) {
        self.asset_fragments = Some(Vec::new());
    }

    pub(crate) fn finish_asset_capture(&mut self) -> Vec<AssetFragment> {
        self.asset_fragments.take().unwrap_or_default()
    }

    fn capture_asset_fragment(&mut self, selector: &str, html: &str) {
        if let Some(fragments) = &mut self.asset_fragments {
            fragments.push(AssetFragment {
                selector: selector.to_owned(),
                html: html.to_owned(),
            });
        }
    }

    /// Run a mutating selector for the provided selector.
    ///
    /// The content of the document will be replaced with the output of the operation.
    pub fn select_mut(
        &mut self,
        selector: &str,
        mut call: impl FnMut(&mut Element<'_, '_>) -> Result<()>,
    ) -> Result<()> {
        let mut buf = Vec::new();
        let selector: Cow<Selector> = Cow::Owned(selector.parse()?);
        HtmlRewriter::new(
            Settings::new().append_element_content_handler((
                selector,
                ElementContentHandlers::default().element(move |el: &mut Element| {
                    call(el)?;
                    Ok(())
                }),
            )),
            |out: &[u8]| buf.extend_from_slice(out),
        )
        .write(self.html.as_slice())?;

        self.html = buf;

        Ok(())
    }

    /// Run a non-mutating handler for the provided selector
    ///
    /// To perform modifications on the `Document` use `Document::select_mut`.
    pub fn select(
        &self,
        selector: &str,
        mut call: impl FnMut(&Element<'_, '_>) -> Result<()>,
    ) -> Result<()> {
        let selector: Cow<Selector> = Cow::Owned(selector.parse()?);
        HtmlRewriter::new(
            Settings::new().append_element_content_handler((
                selector,
                ElementContentHandlers::default().element(move |el: &mut Element| {
                    call(el)?;
                    Ok(())
                }),
            )),
            |_: &[u8]| {},
        )
        .write(self.html.as_slice())?;

        Ok(())
    }

    /// Will silently fail when attempting to append to [Void Element](https://developer.mozilla.org/en-US/docs/Glossary/Void_element).
    pub fn append_html(&mut self, selector: &str, html: &str) -> Result<()> {
        self.select_mut(selector, |el| {
            el.append(html, lol_html::html_content::ContentType::Html);
            Ok(())
        })?;
        self.capture_asset_fragment(selector, html);
        Ok(())
    }

    pub fn replace_with_html(&mut self, selector: &str, html: &str) -> Result<()> {
        self.select_mut(selector, |el| {
            el.replace(html, lol_html::html_content::ContentType::Html);
            Ok(())
        })?;
        self.capture_asset_fragment(selector, html);
        Ok(())
    }

    pub fn remove(&mut self, selector: &str) -> Result<()> {
        self.select_mut(selector, |el| {
            el.remove();
            Ok(())
        })
    }

    pub fn len(&mut self, selector: &str) -> Result<usize> {
        let mut len = 0;
        self.select(selector, |_| {
            len += 1;
            Ok(())
        })?;

        Ok(len)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn captures_exact_generated_markup_and_insertion_location() {
        let mut doc = Document::new(
            "<html><head></head><body><link id=\"slot\"></body></html>",
            Default::default(),
        )
        .expect("valid HTML");
        doc.start_asset_capture();
        doc.append_html(
            "html head",
            r#"<style nonce="nonce-value">body{color:red}</style>"#,
        )
        .expect("append inline stylesheet");
        doc.replace_with_html(
            "link#slot",
            r#"<script type="module" nonce="nonce-value">init()</script>"#,
        )
        .expect("replace asset slot");

        assert_eq!(
            doc.finish_asset_capture(),
            [
                AssetFragment {
                    selector: "html head".to_owned(),
                    html: r#"<style nonce="nonce-value">body{color:red}</style>"#.to_owned(),
                },
                AssetFragment {
                    selector: "link#slot".to_owned(),
                    html: r#"<script type="module" nonce="nonce-value">init()</script>"#.to_owned(),
                },
            ]
        );
    }

    /// Run some basic tests with a spec-compliant HTML file.
    ///
    /// The focus is on the `<script>` element, and around other self-closing elements. If a
    /// self-closing script tag is being used which, according to the spec should not be used, bad
    /// things may happen. This test is there to test our expectation towards a spec-compliant file.
    #[test]
    fn test_script_spec() {
        let mut doc = Document::new(
            r#"
<html>
    <head>
        <link/>
        <script href="test"></script>
        <link>
    </head>
    <body></body>
</html>
"#,
            Default::default(),
        )
        .expect("this is valid HTML");

        doc.append_html("script", r#"<span>here</span>"#)
            .expect("not expected to fail");

        let doc = String::from_utf8_lossy(doc.as_ref());

        assert_eq!(
            doc,
            r#"
<html>
    <head>
        <link/>
        <script href="test"><span>here</span></script>
        <link>
    </head>
    <body></body>
</html>
"#
        );
    }

    /// Ensure we get an error for any self-closing script tag
    #[test]
    fn test_self_closing_script_tag() {
        let doc = Document::new("<script/>", Default::default());
        assert!(doc.is_err());
    }

    /// Ensure we get an error for a self-closing trunk script tag.
    #[test]
    fn test_self_closing_trunk_script_tag() {
        let doc = Document::new("<script data-trunk/>", Default::default());
        assert!(doc.is_err());
    }
}
