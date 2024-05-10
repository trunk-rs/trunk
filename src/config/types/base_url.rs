use reqwest::Url;
use schemars::gen::SchemaGenerator;
use schemars::schema::{Schema, SchemaObject};
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::convert::Infallible;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BaseUrl {
    #[default]
    Default,
    Absolute(Url),
    AbsolutePath(String),
    RelativePath(String),
}

impl BaseUrl {
    pub(crate) fn fix_trailing_slash(self) -> Self {
        match self {
            Self::Absolute(mut url) if !url.path().ends_with('/') => {
                url.set_path(&format!("{}/", url.path()));
                Self::Absolute(url)
            }
            Self::AbsolutePath(path) if !path.ends_with('/') => {
                Self::AbsolutePath(format!("{path}/"))
            }
            Self::RelativePath(path) if !path.ends_with('/') => {
                Self::RelativePath(format!("{path}/"))
            }
            _ => self,
        }
    }
}

impl FromStr for BaseUrl {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s.is_empty() {
            Self::Default
        } else if s.starts_with('/') {
            Self::AbsolutePath(s.to_string())
        } else if let Ok(url) = Url::parse(s) {
            Self::Absolute(url)
        } else {
            Self::RelativePath(s.to_string())
        })
    }
}

impl<'de> Deserialize<'de> for BaseUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BaseUrl::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for BaseUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl JsonSchema for BaseUrl {
    fn schema_name() -> String {
        "BaseUrl".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let mut schema: SchemaObject = String::json_schema(gen).into();

        schema.format = Some("uri".into());

        schema.into()
    }
}

impl AsRef<str> for BaseUrl {
    fn as_ref(&self) -> &str {
        match self {
            Self::Default => "/",
            Self::Absolute(url) => url.as_str(),
            Self::AbsolutePath(url) => url,
            Self::RelativePath(url) => url,
        }
    }
}

impl AsRef<OsStr> for BaseUrl {
    fn as_ref(&self) -> &OsStr {
        match self {
            Self::Default => "/".as_ref(),
            Self::Absolute(url) => url.as_ref().as_ref(),
            Self::AbsolutePath(url) => url.as_ref(),
            Self::RelativePath(url) => url.as_ref(),
        }
    }
}

impl Display for BaseUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => f.write_str("/"),
            Self::Absolute(url) => f.write_str(url.as_ref()),
            Self::AbsolutePath(url) => f.write_str(url),
            Self::RelativePath(url) => f.write_str(url),
        }
    }
}

impl Deref for BaseUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Default => "/",
            Self::Absolute(url) => url.as_ref(),
            Self::AbsolutePath(url) => url,
            Self::RelativePath(url) => url,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::config::types::BaseUrl;
    use reqwest::Url;

    #[test]
    fn test_parse_empty() {
        let base = "".parse();
        assert_eq!(base, Ok(BaseUrl::Default))
    }

    #[test]
    fn test_parse_relative() {
        let base = "./foo".parse();
        assert_eq!(base, Ok(BaseUrl::RelativePath("./foo".to_string())))
    }

    #[test]
    fn test_parse_relative_2() {
        let base = "foo".parse();
        assert_eq!(base, Ok(BaseUrl::RelativePath("foo".to_string())))
    }

    #[test]
    fn test_parse_absolute_path() {
        let base = "/foo".parse();
        assert_eq!(base, Ok(BaseUrl::AbsolutePath("/foo".to_string())))
    }

    #[test]
    fn test_parse_absolute_url() {
        let base = "https://example.com/foo".parse();
        assert_eq!(
            base,
            Ok(BaseUrl::Absolute(
                Url::parse("https://example.com/foo").expect("known url must parse")
            ))
        )
    }

    #[test]
    fn test_fix_trailing_slash() {
        let base = "https://example.com/foo"
            .parse::<BaseUrl>()
            .expect("must parse");
        assert_eq!(
            base.fix_trailing_slash(),
            BaseUrl::Absolute(
                Url::parse("https://example.com/foo/").expect("known url must parse")
            )
        )
    }
}
