use minify_js::TopLevelMode;

/// perform JS minification
pub fn minify_js(bytes: Vec<u8>, mode: TopLevelMode) -> Vec<u8> {
    let mut result: Vec<u8> = vec![];
    let session = minify_js::Session::new();

    match minify_js::minify(&session, mode, &bytes, &mut result) {
        Ok(()) => result,
        Err(err) => {
            tracing::warn!("Failed to minify JS: {err}");
            bytes
        }
    }
}

/// perform CSS minification
pub fn minify_css(bytes: Vec<u8>) -> Vec<u8> {
    use lightningcss::stylesheet::*;

    /// wrap CSS minification to isolate borrowing the original content
    fn minify(css: &str) -> Result<String, ()> {
        // parse CSS

        let mut css = StyleSheet::parse(css, ParserOptions::default()).map_err(|err| {
            tracing::warn!("CSS parsing failed, skipping: {err}");
        })?;

        css.minify(MinifyOptions::default()).map_err(|err| {
            tracing::warn!("CSS minification failed, skipping: {err}");
        })?;

        Ok(css
            .to_css(PrinterOptions {
                minify: true,
                ..Default::default()
            })
            .map_err(|err| {
                tracing::warn!("CSS generation failed, skipping: {err}");
            })?
            .code)
    }

    match std::str::from_utf8(&bytes) {
        Ok(css) => minify(css).map(String::into_bytes).unwrap_or(bytes),
        Err(_) => bytes,
    }
}

/// perform HTML minification
pub fn minify_html(html: &[u8]) -> Vec<u8> {
    let mut minify_cfg = minify_html::Cfg::spec_compliant();
    minify_cfg.minify_css = true;
    minify_cfg.minify_js = true;
    minify_cfg.keep_closing_tags = true;
    minify_html::minify(html, &minify_cfg)
}
