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
    use css_minify::optimizations::*;

    if let Ok(css) = std::str::from_utf8(&bytes) {
        match Minifier::default().minify(css, Level::One) {
            Ok(result) => return result.into_bytes(),
            Err(err) => {
                tracing::warn!("Failed to minify CSS: {err}");
            }
        }
    }

    bytes
}

/// perform HTML minification
pub fn minify_html(html: &[u8]) -> Vec<u8> {
    let mut minify_cfg = minify_html::Cfg::spec_compliant();
    minify_cfg.minify_css = true;
    minify_cfg.minify_js = true;
    minify_cfg.keep_closing_tags = true;
    minify_html::minify(html, &minify_cfg)
}
