//! Verification of package tarballs against the digests announced by the registry.
//!
//! This follows what npm does (see `ssri`): the `dist.integrity` field contains one or more
//! [subresource integrity](https://www.w3.org/TR/SRI/) entries, separated by whitespace. Entries
//! with unsupported algorithms are ignored, and only the entries of the strongest algorithm are
//! considered. A tarball is accepted if it matches any of them. If there is no usable integrity
//! entry, the legacy `dist.shasum` field (a hex encoded SHA-1 digest) is used instead.

use base64::{DecodeError, Engine, engine::general_purpose::STANDARD};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

/// Supported digest algorithms, ordered from weakest to strongest.
///
/// Parsed from, and displayed as, the SRI algorithm names (e.g. `sha512`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Algorithm {
    fn output_len(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => Vec::from_iter(Sha1::digest(data)),
            Self::Sha256 => Vec::from_iter(Sha256::digest(data)),
            Self::Sha384 => Vec::from_iter(Sha384::digest(data)),
            Self::Sha512 => Vec::from_iter(Sha512::digest(data)),
        }
    }
}

/// Errors of verifying a package tarball.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The registry announced no usable digest, so the package can not be verified.
    #[error("the registry provides neither an integrity value nor a shasum")]
    Missing,
    /// A digest is not valid base64.
    #[error("malformed {algorithm} digest, invalid base64: {value}")]
    InvalidBase64 {
        algorithm: Algorithm,
        value: String,
        #[source]
        source: DecodeError,
    },
    /// The shasum is not valid hex.
    #[error("malformed shasum, invalid hex: {value}")]
    InvalidHex { value: String },
    /// A digest does not have the length of the output of its algorithm.
    #[error(
        "{algorithm} digest has the wrong length, expected {expected} bytes, got {actual}: {value}"
    )]
    WrongLength {
        algorithm: Algorithm,
        value: String,
        expected: usize,
        actual: usize,
    },
    /// The tarball does not match any of the expected digests.
    #[error("integrity check failed: expected {}, got {actual}", expected.join(" or "))]
    Mismatch {
        /// The expected digests, in SRI format.
        expected: Vec<String>,
        /// The actual digest, in SRI format.
        actual: String,
    },
}

/// The digests a package tarball must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Integrity {
    algorithm: Algorithm,
    /// Acceptable digests, all using `algorithm`. Never empty.
    digests: Vec<Vec<u8>>,
}

impl Integrity {
    /// Get the expected digests from the `dist.integrity` and `dist.shasum` fields of a package.
    ///
    /// Fails if neither of them contains a usable digest, so that a package is never installed
    /// without verifying it.
    pub fn from_distribution(integrity: Option<&str>, shasum: Option<&str>) -> Result<Self, Error> {
        if let Some(integrity) = Self::parse_integrity(integrity.unwrap_or_default())? {
            return Ok(integrity);
        }

        match shasum.map(str::trim).filter(|shasum| !shasum.is_empty()) {
            Some(shasum) => {
                let digest = decode_hex(shasum).ok_or_else(|| Error::InvalidHex {
                    value: shasum.to_string(),
                })?;
                Ok(Self {
                    algorithm: Algorithm::Sha1,
                    digests: vec![check_length(Algorithm::Sha1, digest, shasum)?],
                })
            }
            None => Err(Error::Missing),
        }
    }

    fn parse_integrity(value: &str) -> Result<Option<Self>, Error> {
        let mut entries = Vec::new();
        for entry in value.split_whitespace() {
            let Some((algorithm, digest)) = entry.split_once('-') else {
                tracing::debug!("ignoring malformed integrity entry: {entry}");
                continue;
            };
            let algorithm = match Algorithm::try_from(algorithm) {
                Ok(algorithm) => algorithm,
                Err(err) => {
                    // As npm does, entries of unknown algorithms are ignored. If none is left,
                    // the shasum is used, or the package is rejected.
                    tracing::debug!("ignoring integrity entry {entry}: {algorithm}: {err}");
                    continue;
                }
            };
            // Drop options, which are allowed by the SRI syntax, but carry no meaning.
            let digest = digest.split_once('?').map_or(digest, |(digest, _)| digest);
            let digest = STANDARD
                .decode(digest)
                .map_err(|source| Error::InvalidBase64 {
                    algorithm,
                    value: entry.to_string(),
                    source,
                })?;
            entries.push((algorithm, check_length(algorithm, digest, entry)?));
        }

        let Some(algorithm) = entries.iter().map(|(algorithm, _)| *algorithm).max() else {
            return Ok(None);
        };
        let digests = entries
            .into_iter()
            .filter(|(a, _)| *a == algorithm)
            .map(|(_, digest)| digest)
            .collect();

        Ok(Some(Self { algorithm, digests }))
    }

    /// Verify that `data` matches one of the expected digests.
    pub fn verify(&self, data: &[u8]) -> Result<(), Error> {
        let actual = self.algorithm.digest(data);
        if self.digests.contains(&actual) {
            return Ok(());
        }

        Err(Error::Mismatch {
            expected: self.digests.iter().map(|d| self.sri(d)).collect(),
            actual: self.sri(&actual),
        })
    }

    /// Format a digest in SRI format.
    fn sri(&self, digest: &[u8]) -> String {
        format!("{}-{}", self.algorithm, STANDARD.encode(digest))
    }
}

/// Check that a decoded digest has the length of the output of its algorithm.
fn check_length(algorithm: Algorithm, digest: Vec<u8>, value: &str) -> Result<Vec<u8>, Error> {
    if digest.len() == algorithm.output_len() {
        Ok(digest)
    } else {
        Err(Error::WrongLength {
            algorithm,
            value: value.to_string(),
            expected: algorithm.output_len(),
            actual: digest.len(),
        })
    }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.bytes().all(|b| b.is_ascii_hexdigit()) || !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &[u8] = b"package";

    fn sri(algorithm: Algorithm, data: &[u8]) -> String {
        format!("{algorithm}-{}", STANDARD.encode(algorithm.digest(data)))
    }

    fn shasum(data: &[u8]) -> String {
        Algorithm::Sha1
            .digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn accepts_matching_integrity() -> anyhow::Result<()> {
        for algorithm in [
            Algorithm::Sha1,
            Algorithm::Sha256,
            Algorithm::Sha384,
            Algorithm::Sha512,
        ] {
            let integrity = Integrity::from_distribution(Some(&sri(algorithm, DATA)), None)?;
            integrity.verify(DATA)?;
            assert!(
                matches!(integrity.verify(b"tampered"), Err(Error::Mismatch { .. })),
                "{algorithm}"
            );
        }
        Ok(())
    }

    #[test]
    fn uses_strongest_algorithm() -> anyhow::Result<()> {
        // A matching weaker digest must not make up for a mismatching stronger one.
        let value = format!(
            "{} {}",
            sri(Algorithm::Sha1, DATA),
            sri(Algorithm::Sha512, b"other")
        );
        let integrity = Integrity::from_distribution(Some(&value), Some(&shasum(DATA)))?;
        let Err(Error::Mismatch { expected, actual }) = integrity.verify(DATA) else {
            panic!("expected a mismatch");
        };
        assert_eq!(expected, [sri(Algorithm::Sha512, b"other")]);
        assert_eq!(actual, sri(Algorithm::Sha512, DATA));
        Ok(())
    }

    #[test]
    fn accepts_any_digest_of_strongest_algorithm() -> anyhow::Result<()> {
        let value = format!(
            "{} {}?opt",
            sri(Algorithm::Sha512, b"other"),
            sri(Algorithm::Sha512, DATA)
        );
        Integrity::from_distribution(Some(&value), None)?.verify(DATA)?;
        Ok(())
    }

    #[test]
    fn parses_algorithm_names() -> anyhow::Result<()> {
        for algorithm in [
            Algorithm::Sha1,
            Algorithm::Sha256,
            Algorithm::Sha384,
            Algorithm::Sha512,
        ] {
            assert_eq!(
                Algorithm::try_from(algorithm.to_string().as_str())?,
                algorithm
            );
        }
        assert!(Algorithm::try_from("md5").is_err());
        assert!(Algorithm::try_from("SHA512").is_err());
        assert!(Algorithm::try_from("").is_err());
        Ok(())
    }

    #[test]
    fn ignores_unknown_algorithms() -> anyhow::Result<()> {
        let value = format!("md5-AAAA {}", sri(Algorithm::Sha256, DATA));
        let integrity = Integrity::from_distribution(Some(&value), None)?;
        assert_eq!(integrity.algorithm, Algorithm::Sha256);
        integrity.verify(DATA)?;
        Ok(())
    }

    #[test]
    fn falls_back_to_shasum() -> anyhow::Result<()> {
        for integrity in [None, Some(""), Some("md5-AAAA")] {
            let integrity = Integrity::from_distribution(integrity, Some(&shasum(DATA)))?;
            integrity.verify(DATA)?;
            assert!(matches!(
                integrity.verify(b"tampered"),
                Err(Error::Mismatch { .. })
            ));
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_digests() -> anyhow::Result<()> {
        assert!(matches!(
            Integrity::from_distribution(None, None),
            Err(Error::Missing)
        ));
        assert!(matches!(
            Integrity::from_distribution(Some(" "), Some("")),
            Err(Error::Missing)
        ));
        Ok(())
    }

    #[test]
    fn rejects_malformed_digests() -> anyhow::Result<()> {
        assert!(matches!(
            Integrity::from_distribution(Some("sha512-!!!"), None),
            Err(Error::InvalidBase64 {
                algorithm: Algorithm::Sha512,
                ..
            })
        ));
        assert!(matches!(
            Integrity::from_distribution(Some("sha512-AAAA"), None),
            Err(Error::WrongLength {
                algorithm: Algorithm::Sha512,
                expected: 64,
                actual: 3,
                ..
            })
        ));
        assert!(matches!(
            Integrity::from_distribution(None, Some("xyz")),
            Err(Error::InvalidHex { .. })
        ));
        assert!(matches!(
            Integrity::from_distribution(None, Some("abcd")),
            Err(Error::WrongLength {
                algorithm: Algorithm::Sha1,
                expected: 20,
                actual: 2,
                ..
            })
        ));
        Ok(())
    }
}
