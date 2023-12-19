use anyhow::anyhow;
use minify_js::TopLevelMode;

/// perform JS minification
pub fn minify_js(bytes: &[u8], mode: TopLevelMode) -> anyhow::Result<Vec<u8>> {
    let mut result: Vec<u8> = vec![];
    let session = minify_js::Session::new();
    minify_js::minify(&session, mode, bytes, &mut result)
        .map_err(|err| anyhow!("Failed to minify JS: {err}"))?;

    Ok(result)
}

/// perform CSS minification
pub fn minify_css(bytes: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    use css_minify::optimizations::*;

    if let Ok(css) = std::str::from_utf8(&bytes) {
        Ok(Minifier::default()
            .minify(css, Level::Three)
            .map_err(|err| anyhow!("Failed to minify CSS: {err}"))?
            .into_bytes())
    } else {
        Ok(bytes)
    }
}

/// perform HTML minification
pub fn minify_html(html: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut minify_cfg = minify_html::Cfg::spec_compliant();
    minify_cfg.minify_css = true;
    minify_cfg.minify_js = true;
    minify_cfg.keep_closing_tags = true;
    Ok(minify_html::minify(html, &minify_cfg))
}
