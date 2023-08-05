pub(crate) use trunk_util::*;

pub(crate) const ATTR_INLINE: &str = "data-inline";
pub(crate) const ATTR_HREF: &str = "href";
pub(crate) const ATTR_SRC: &str = "src";
pub(crate) const ATTR_TYPE: &str = "type";
pub(crate) const ATTR_REL: &str = "rel";
pub(crate) const SNIPPETS_DIR: &str = "snippets";
pub(crate) const TRUNK_ID: &str = "data-trunk-id";

/// Create the CSS selector for selecting a trunk link by ID.
pub(crate) fn trunk_id_selector(id: usize) -> String {
    format!(r#"link[{}="{}"]"#, TRUNK_ID, id)
}

/// Create the CSS selector for selecting a trunk script by ID.
pub(crate) fn trunk_script_id_selector(id: usize) -> String {
    format!(r#"script[{}="{}"]"#, TRUNK_ID, id)
}
