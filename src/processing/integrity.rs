//! Integrity processing

use crate::config::Integrity;
use base64::display::Base64Display;
use base64::engine::general_purpose::URL_SAFE;
use sha2::{Digest, Sha256, Sha384, Sha512};

/// The digest of the output
#[derive(Debug)]
pub struct OutputDigest {
    /// The digest algorithm
    pub integrity: Integrity,
    /// The raw hash/digest value
    pub hash: Vec<u8>,
}

impl Default for OutputDigest {
    fn default() -> Self {
        Self {
            integrity: Integrity::None,
            hash: vec![],
        }
    }
}

impl OutputDigest {
    /// Turn into a SRI attribute
    pub fn make_attribute(&self) -> String {
        match self.integrity {
            Integrity::None => String::default(),
            integrity => {
                // format of an attribute, including the leading space
                format!(
                    r#" integrity="{integrity}-{hash}""#,
                    hash = Base64Display::new(&self.hash, &URL_SAFE)
                )
            }
        }
    }

    /// Generate from input data
    pub fn generate(integrity: Integrity, data: &[u8]) -> Self {
        let hash = match integrity {
            Integrity::None => vec![],
            Integrity::Sha256 => Vec::from_iter(Sha256::digest(data)),
            Integrity::Sha384 => Vec::from_iter(Sha384::digest(data)),
            Integrity::Sha512 => Vec::from_iter(Sha512::digest(data)),
        };

        Self { integrity, hash }
    }
}
