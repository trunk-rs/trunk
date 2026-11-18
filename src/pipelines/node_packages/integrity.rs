//! Verification of package tarballs against the digests announced by the registry.
//!
//! This follows what npm does (see `ssri`): the `dist.integrity` field contains one or more
//! [subresource integrity](https://www.w3.org/TR/SRI/) entries, separated by whitespace. Entries
//! with unsupported algorithms are ignored, and only the entries of the strongest algorithm are
//! considered. A tarball is accepted if it matches any of them. If there is no usable integrity
//! entry, the legacy `dist.shasum` field (a hex encoded SHA-1 digest) is used instead.

use anyhow::bail;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::fmt::{Display, Formatter};

/// Supported digest algorithms, ordered from weakest to strongest.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Algorithm {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Algorithm {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "sha1" => Self::Sha1,
            "sha256" => Self::Sha256,
            "sha384" => Self::Sha384,
            "sha512" => Self::Sha512,
            _ => return None,
        })
    }

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

impl Display for Algorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha384 => "sha384",
            Self::Sha512 => "sha512",
        })
    }
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
    pub fn from_distribution(
        integrity: Option<&str>,
        shasum: Option<&str>,
    ) -> anyhow::Result<Self> {
        if let Some(integrity) = Self::parse_integrity(integrity.unwrap_or_default())? {
            return Ok(integrity);
        }

        match shasum.map(str::trim).filter(|shasum| !shasum.is_empty()) {
            Some(shasum) => Ok(Self {
                algorithm: Algorithm::Sha1,
                digests: vec![check(Algorithm::Sha1, decode_hex(shasum), shasum)?],
            }),
            None => bail!("the registry provides neither an integrity value nor a shasum"),
        }
    }

    fn parse_integrity(value: &str) -> anyhow::Result<Option<Self>> {
        let mut entries = Vec::new();
        for entry in value.split_whitespace() {
            let Some((algorithm, digest)) = entry.split_once('-') else {
                continue;
            };
            let Some(algorithm) = Algorithm::parse(algorithm) else {
                continue;
            };
            // Drop options, which are allowed by the SRI syntax, but carry no meaning.
            let digest = digest.split_once('?').map_or(digest, |(digest, _)| digest);
            entries.push((
                algorithm,
                check(algorithm, STANDARD.decode(digest).ok(), entry)?,
            ));
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
    pub fn verify(&self, data: &[u8]) -> anyhow::Result<()> {
        let actual = self.algorithm.digest(data);
        if self.digests.contains(&actual) {
            return Ok(());
        }

        bail!(
            "integrity check failed: expected {}, got {}-{}",
            self.digests
                .iter()
                .map(|digest| format!("{}-{}", self.algorithm, STANDARD.encode(digest)))
                .collect::<Vec<_>>()
                .join(" or "),
            self.algorithm,
            STANDARD.encode(actual)
        )
    }
}

/// Check the result of decoding a digest, including its length.
fn check(algorithm: Algorithm, digest: Option<Vec<u8>>, raw: &str) -> anyhow::Result<Vec<u8>> {
    match digest {
        Some(digest) if digest.len() == algorithm.output_len() => Ok(digest),
        Some(_) => bail!("{algorithm} digest has the wrong length: {raw}"),
        None => bail!("malformed {algorithm} digest: {raw}"),
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
            assert!(integrity.verify(b"tampered").is_err(), "{algorithm}");
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
        assert!(integrity.verify(DATA).is_err());
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
            assert!(integrity.verify(b"tampered").is_err());
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_digests() -> anyhow::Result<()> {
        assert!(Integrity::from_distribution(None, None).is_err());
        assert!(Integrity::from_distribution(Some(" "), Some("")).is_err());
        Ok(())
    }

    #[test]
    fn rejects_malformed_digests() -> anyhow::Result<()> {
        assert!(Integrity::from_distribution(Some("sha512-!!!"), None).is_err());
        assert!(Integrity::from_distribution(Some("sha512-AAAA"), None).is_err());
        assert!(Integrity::from_distribution(None, Some("xyz")).is_err());
        assert!(Integrity::from_distribution(None, Some("abcd")).is_err());
        Ok(())
    }
}
