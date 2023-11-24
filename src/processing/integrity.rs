//! Integrity processing

use base64::{display::Base64Display, engine::general_purpose::URL_SAFE};
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Integrity type for subresource protection
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum IntegrityType {
    None,
    Sha256,
    #[default]
    Sha384,
    Sha512,
}

impl FromStr for IntegrityType {
    type Err = IntegrityTypeParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "" => Default::default(),
            "none" => Self::None,
            "sha256" => Self::Sha256,
            "sha384" => Self::Sha384,
            "sha512" => Self::Sha512,
            _ => return Err(IntegrityTypeParseError::InvalidValue),
        })
    }
}

impl Display for IntegrityType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Sha256 => write!(f, "sha256"),
            Self::Sha384 => write!(f, "sha384"),
            Self::Sha512 => write!(f, "sha512"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrityTypeParseError {
    #[error("invalid value")]
    InvalidValue,
}

/// The digest of the output
#[derive(Debug)]
pub struct OutputDigest {
    /// The digest algorithm
    pub integrity: IntegrityType,
    /// The raw hash/digest value
    pub hash: Vec<u8>,
}

impl Default for OutputDigest {
    fn default() -> Self {
        Self {
            integrity: IntegrityType::None,
            hash: vec![],
        }
    }
}

impl OutputDigest {
    /// Turn into a SRI attribute with can be appended to a string.
    pub fn make_attribute(&self) -> String {
        self.to_integrity_value()
            .map(|value| {
                // format of an attribute, including the leading space
                format!(r#" integrity="{value}""#)
            })
            .unwrap_or_default()
    }

    /// Turn into the value for an SRI attribute
    pub fn to_integrity_value(&self) -> Option<impl Display + '_> {
        match self.integrity {
            IntegrityType::None => None,
            integrity => Some(format!(
                "{integrity}-{hash}",
                hash = Base64Display::new(&self.hash, &URL_SAFE)
            )),
        }
    }

    /// Insert as an SRI attribute into a an [`Attrs`] instance.
    pub fn insert_into(&self, attrs: &mut HashMap<String, String>) {
        if let Some(value) = self.to_integrity_value() {
            attrs.insert("integrity".to_string(), value.to_string());
        }
    }

    /// Generate from input data
    pub fn generate<F, T, E>(integrity: IntegrityType, f: F) -> Result<Self, E>
    where
        F: FnOnce() -> Result<T, E>,
        T: AsRef<[u8]>,
    {
        let hash = match integrity {
            IntegrityType::None => vec![],
            IntegrityType::Sha256 => Vec::from_iter(Sha256::digest(f()?)),
            IntegrityType::Sha384 => Vec::from_iter(Sha384::digest(f()?)),
            IntegrityType::Sha512 => Vec::from_iter(Sha512::digest(f()?)),
        };

        Ok(Self { integrity, hash })
    }

    /// Generate from existing input data
    pub fn generate_from(integrity: IntegrityType, data: impl AsRef<[u8]>) -> Self {
        Self::generate::<_, _, Infallible>(integrity, || Ok(data))
            // we can safely unwrap, as we know it's infallible
            .unwrap()
    }
}
