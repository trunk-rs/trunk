use crate::config::{
    models::ConfigModel,
    types::{BaseUrl, Minify},
};
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    marker::PhantomData,
    path::PathBuf,
    str::FromStr,
};

/// Config options for the build system.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct Build {
    /// The index HTML file to drive the bundling process
    #[serde(default = "default::target")]
    pub target: PathBuf,

    /// The name of the output HTML file.
    ///
    /// If not set, use the same name as the target HTML file.
    pub html_output: Option<String>,

    /// Build in release mode [default: false]
    #[serde(default)]
    pub release: bool,

    /// Cargo profile to use.
    ///
    /// Overrides the default chosen by cargo. Ignored if the 'index.html' has one configured.
    #[serde(default)]
    pub cargo_profile: Option<String>,

    /// The output dir for all final assets
    #[serde(default = "default::dist")]
    pub dist: PathBuf,

    /// Run without accessing the network
    #[serde(default)]
    pub offline: bool,

    /// Require Cargo.lock and cache are up to date
    #[serde(default)]
    pub frozen: bool,

    /// Require Cargo.lock is up to date
    #[serde(default)]
    pub locked: bool,

    /// The public URL from which assets are to be served
    #[serde(default)]
    pub public_url: BaseUrl,

    /// Don't add a trailing slash to the public URL if it is missing
    #[serde(default)]
    pub public_url_no_trailing_slash_fix: bool,

    /// Build without default features
    #[serde(default)]
    pub no_default_features: bool,

    /// Build with all features
    #[serde(default)]
    pub all_features: bool,

    /// A comma-separated list of features to activate, must not be used with all-features
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "string_or_vec")]
    #[schemars(schema_with = "schema::features")]
    pub features: Vec<String>,

    /// Whether to include hash values in the output file names
    #[serde(default = "default::filehash")]
    pub filehash: bool,

    /// Whether to build an example.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,

    /// Optional pattern for the app loader script [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly load the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_script: Option<String>,

    /// Whether to inject scripts into your index file.
    ///
    /// These values can only be provided via config file.
    #[serde(default = "default::inject_scripts")]
    pub inject_scripts: bool,

    /// Optional pattern for the app preload element [default: None]
    ///
    /// Patterns should include the sequences `{base}`, `{wasm}`, and `{js}` in order to
    /// properly preload the application. Other sequences may be included corresponding
    /// to key/value pairs provided in `pattern_params`.
    ///
    /// These values can only be provided via config file.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_preload: Option<String>,

    /// Optional replacement parameters corresponding to the patterns provided in
    /// `pattern_script` and `pattern_preload`.
    ///
    /// When a pattern is being replaced with its corresponding value from this map, if the value
    /// is prefixed with the symbol `@`, then the value is expected to be a file path, and the
    /// pattern will be replaced with the contents of the target file. This allows insertion of
    /// some big JSON state or even HTML files as a part of the `index.html` build.
    ///
    /// Trunk will automatically insert the `base`, `wasm` and `js` key/values into this map. In
    /// order for the app to be loaded properly, the patterns `{base}`, `{wasm}` and `{js}` should
    /// be used in `pattern_script` and `pattern_preload`.
    ///
    /// These values can only be provided via config file.
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub pattern_params: HashMap<String, String>,

    /// When desired, set a custom root certificate chain (same format as Cargo's config.toml http.cainfo)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_certificate: Option<String>,

    /// Allows request to ignore certificate validation errors.
    ///
    /// Can be useful when behind a corporate proxy.
    #[serde(default)]
    pub accept_invalid_certs: bool,

    /// Control minification.
    #[serde(default)]
    pub minify: Minify,

    /// Allows disabling sub-resource integrity (SRI)
    #[serde(default)]
    pub no_sri: bool,

    /// Ignore error's related to self-closing script elements, and instead issue a warning.
    ///
    /// Since this issue can cause the HTML output to be truncated, only enable this in case you
    /// are sure it is caused due to a false positive.
    #[serde(default)]
    pub allow_self_closing_script: bool,

    /// Create 'nonce' attributes with a placeholder.
    #[serde(default)]
    pub create_nonce: bool,

    /// The placeholder which is used in the 'nonce' attribute.
    #[serde(default = "default::nonce_placeholder")]
    pub nonce_placeholder: String,
}

fn string_or_vec<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: Deserialize<'de> + FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    struct StringOrVec<T>(PhantomData<fn() -> T>);

    impl<'de, T> de::Visitor<'de> for StringOrVec<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("string of vec")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![T::from_str(v).map_err(de::Error::custom)?])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(element) = seq.next_element::<String>()? {
                let value = T::from_str(&element).map_err(de::Error::custom)?;
                vec.push(value);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

impl Default for Build {
    fn default() -> Self {
        Self {
            target: default::target(),
            html_output: None,
            release: false,
            cargo_profile: None,
            dist: default::dist(),
            offline: false,
            frozen: false,
            locked: false,
            public_url: Default::default(),
            public_url_no_trailing_slash_fix: false,
            no_default_features: false,
            all_features: false,
            features: vec![],
            example: None,
            filehash: default::filehash(),
            pattern_script: None,
            inject_scripts: default::inject_scripts(),
            pattern_preload: None,
            pattern_params: Default::default(),
            root_certificate: None,
            accept_invalid_certs: false,
            minify: Default::default(),
            no_sri: false,
            allow_self_closing_script: false,
            create_nonce: false,
            nonce_placeholder: default::nonce_placeholder(),
        }
    }
}

mod default {
    use crate::config::DIST_DIR;
    use std::path::PathBuf;

    pub fn dist() -> PathBuf {
        DIST_DIR.into()
    }

    pub fn target() -> PathBuf {
        "index.html".into()
    }

    pub const fn filehash() -> bool {
        true
    }

    pub const fn inject_scripts() -> bool {
        true
    }

    pub fn nonce_placeholder() -> String {
        "{{__TRUNK NONCE__}}".to_string()
    }
}

mod schema {
    use schemars::schema::{SchemaObject, SubschemaValidation};
    use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};

    pub fn features(gen: &mut SchemaGenerator) -> Schema {
        let schema = SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                one_of: Some(vec![
                    String::json_schema(gen),
                    Vec::<String>::json_schema(gen),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        };

        schema.into()
    }
}

impl ConfigModel for Build {}
