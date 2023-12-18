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
