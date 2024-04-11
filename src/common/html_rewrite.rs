use anyhow::Result;
use lol_html::{element, html_content::Element, HtmlRewriter, Settings};

/// A wrapper for Html modifications, and rewrites.
#[derive(Debug)]
pub struct Document(Vec<u8>);

impl AsRef<[u8]> for Document {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Document {
    /// Create a new document
    ///
    /// Note: if this is not a valid HTML document, it will fail later on.
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self(data.into())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }

    #[inline]
    fn default_settings() -> Settings<'static, 'static> {
        Settings {
            ..Settings::default()
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
        HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!(selector, |x| {
                    call(x)?;
                    Ok(())
                })],
                ..Self::default_settings()
            },
            |out: &[u8]| buf.extend_from_slice(out),
        )
        .write(self.0.as_slice())?;

        self.0 = buf;

        Ok(())
    }

    /// Run a non-mutating handler for the provided selector
    ///
    /// To perform modifications on the `Document` use `Document::select_mut`.
    pub fn select(&self, selector: &str, mut call: impl FnMut(&Element<'_, '_>)) -> Result<()> {
        HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!(selector, |el| {
                    call(el);
                    Ok(())
                })],
                ..Self::default_settings()
            },
            |_: &[u8]| {},
        )
        .write(self.0.as_slice())?;

        Ok(())
    }

    /// Will silently fail when attempting to append to [Void Element](https://developer.mozilla.org/en-US/docs/Glossary/Void_element).
    pub fn append_html(&mut self, selector: &str, html: &str) -> Result<()> {
        self.select_mut(selector, |el| {
            el.append(html, lol_html::html_content::ContentType::Html);
            Ok(())
        })
    }

    pub fn replace_with_html(&mut self, selector: &str, html: &str) -> Result<()> {
        self.select_mut(selector, |el| {
            el.replace(html, lol_html::html_content::ContentType::Html);
            Ok(())
        })?;
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
        self.select(selector, |_| len += 1)?;

        Ok(len)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_script_spec() {
        let mut doc = Document::new(
            r#"
<html>
    <head>
        <script href="test"></script>
        <link>
    </head>
    <body></body>
</html>
"#,
        );

        doc.append_html("script", r#"<span>here</span>"#)
            .expect("not expected to fail");

        let doc = String::from_utf8_lossy(&doc.0);

        assert_eq!(
            doc,
            r#"
<html>
    <head>
        <script href="test"><span>here</span></script>
        <link>
    </head>
    <body></body>
</html>
"#
        );
    }
}
