use std::collections::HashMap;
use std::path::PathBuf;

/// A mapping of all attrs associated with a specific `<... data-trunk .../>` element.
pub type Attrs = HashMap<String, String>;

/// A type that is used as an input to an asset pipeline.
#[derive(Debug, Clone)]
pub struct AssetInput {
    /// The directory where the input manifest (usually: `index.html`) is stored.
    pub manifest_dir: PathBuf,
    /// The name of the tag, link, style, script, ...
    pub tag_name: String,
    /// The attribute of the asset, stored in a HashMap.
    pub attrs: Attrs,
    /// The ID assigned to the asset tag.
    pub id: usize,
    // TODO: content of the tag?
}
