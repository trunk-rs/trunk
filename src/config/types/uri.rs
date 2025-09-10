use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer};
use std::{borrow::Cow, ops::Deref, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Uri(
    #[serde(deserialize_with = "crate::config::types::deserialize_uri")] pub axum::http::Uri,
);

impl JsonSchema for Uri {
    fn schema_name() -> Cow<'static, str> {
        "Uri".into()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "uri",
        })
    }
}

impl Deref for Uri {
    type Target = axum::http::Uri;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<axum::http::Uri> for Uri {
    fn from(value: axum::http::Uri) -> Self {
        Self(value)
    }
}

/// Deserialize a Uri from a string.
pub fn deserialize_uri<'de, D, T>(data: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: From<axum::http::Uri>,
{
    let val = String::deserialize(data)?;
    axum::http::Uri::from_str(val.as_str())
        .map(Into::into)
        .map_err(|err| serde::de::Error::custom(err.to_string()))
}

#[cfg(test)]
mod test {
    use serde_json::json;

    fn assert_uri(uri: &str) {
        assert_eq!(
            serde_json::from_value::<super::Uri>(json!(uri))
                .expect("must parse")
                .to_string(),
            uri
        );
    }

    #[test]
    fn deserialize() {
        assert_uri("/foo");
        assert_uri("https://localhost/foo");
    }
}
