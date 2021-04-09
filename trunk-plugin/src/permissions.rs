use std::collections::HashMap;

bitflags::bitflags! {
    /// A set of permission a trunk plugin can have.
    /// 
    /// For security reasons a plugin is really restricted by default. But the user can
    /// allow the plugin to perform certain actions. This struct provides a method of
    /// storing these permissions.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    pub struct Permissions: u8 {
        /// The plugin has no permissions.
        const NONE                          = 0b00000000;
        /// The plugin may read the index.html document.
        const READ_HTML_DOCUMENT            = 0b00000001;
        /// The plugin may replace the html node that called 
        /// it with arbitrary html.
        const REPLACE_HTML_DOCUMENT_CALL    = 0b00000010;
        /// The plugin may create files in the output directory.
        const CREATE_OUTPUT_FILE            = 0b00000100;
        /// The plugin may read arbitrary files from the html dir.
        const READ_HTML_DIR_FILE            = 0b00001000;
        /// The plugin may call another plugin.
        /// The called plugin will receive it's arguments from
        /// the calling plugin and will inherit all permissions
        /// from the calling plugin except for this permission
        /// (to prevent cycles). 
        const CALL_PLUGIN                   = 0b00010000;
    }
}

impl Permissions {
    /// The name of an HTML attribute containing a comma-separated list of permissions.
    const PERMISSION_LIST_ATTR: &'static str = "data-permissions";
    /// The prefix of HTML permission-attributes.
    const PERMISSION_ATTR_PREFIX: &'static str = "data-permission-";

    /// This method takes a HashMap of HTML attributes, removes all permission-attributes
    /// form it, and returns a composition of all specified permissions.
    /// 
    /// There are two kind of permission attributes:
    /// 1. The `data-permissions` attribute:  
    ///     This attribute contains a comma separated list of permissions.
    /// 2. `data-permission-<PERMISSION>` attributes:  
    ///     These attributes represent one permission each.
    /// 
    /// Supplying the none-permission or specifying permissions multiple times has no special effect.
    /// The returned Permissions struct will have each permission-flag that was specified set.
    pub fn from_link_attrs(link_attrs: &mut HashMap<String, String>) -> Self {
        let mut permissions = Self::NONE;

        if let Some(permission_list) = link_attrs.remove(Self::PERMISSION_LIST_ATTR) {
            permission_list
                .split_terminator(',')
                .map(str::trim)
                .map(Self::from_flag_name)
                .flatten()
                .for_each(|permission| permissions.insert(permission));
        }

        link_attrs
            .retain(|k, _| {
                k
                    .starts_with(Self::PERMISSION_ATTR_PREFIX)
                    .then(|| &k[Self::PERMISSION_ATTR_PREFIX.len()..])
                    .map(Self::from_flag_name)
                    .flatten()
                    .map(|permission| permissions.insert(permission))
                    .is_none()
            });

        permissions
    }

    /// Tries to parse a str to a permission flag.
    pub fn from_flag_name(name: &str) -> Option<Self> {
        let flag = match name {
            "none" => Self::NONE,
            "read-html-document" => Self::READ_HTML_DOCUMENT,
            "replace-html-document-call" => Self::REPLACE_HTML_DOCUMENT_CALL,
            "create-output-file" => Self::CREATE_OUTPUT_FILE,
            "read-html-dir-file" => Self::READ_HTML_DIR_FILE,
            "call-plugin" => Self::CALL_PLUGIN,
            _ => return None
        };

        Some(flag)
    }
}
