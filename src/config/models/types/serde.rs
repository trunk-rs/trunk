use axum::http::Uri;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

/// Deserialize a Uri from a string.
pub fn deserialize_uri<'de, D, T>(data: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: From<Uri>,
{
    let val = String::deserialize(data)?;
    Uri::from_str(val.as_str())
        .map(Into::into)
        .map_err(|err| serde::de::Error::custom(err.to_string()))
}
