use anyhow::bail;
use flate2::{
    Compression,
    bufread::{DeflateEncoder, GzEncoder, ZlibEncoder},
};
use std::{
    io::{BufRead, Read},
    str::FromStr,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, strum::Display)]
pub enum CompressionAlgorithm {
    #[default]
    #[strum(serialize = "none")]
    None,
    #[strum(serialize = "gzip")]
    Gzip,
    #[strum(serialize = "deflate")]
    Zlib,
    #[strum(serialize = "deflate-raw")]
    Deflate,
}

impl CompressionAlgorithm {
    pub fn from_attrs(attrs: &crate::pipelines::Attrs) -> anyhow::Result<Self> {
        let algorithm = attrs
            .get("data-compression-algorithm")
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or_default();
        if algorithm == Self::None && attrs.contains_key("data-compression-level") {
            tracing::warn!(
                "data-compression-level is unsupported without a compression algorithm; leaving compression disabled"
            );
        }
        Ok(algorithm)
    }

    pub fn encoder<'a, R: BufRead + Send + 'a>(
        &self,
        reader: R,
        level: Compression,
    ) -> Box<dyn Read + Send + 'a> {
        match self {
            Self::None => Box::new(reader),
            Self::Gzip => Box::new(GzEncoder::new(reader, level)),
            Self::Zlib => Box::new(ZlibEncoder::new(reader, level)),
            Self::Deflate => Box::new(DeflateEncoder::new(reader, level)),
        }
    }
}

impl FromStr for CompressionAlgorithm {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "gzip" => Ok(Self::Gzip),
            "deflate" => Ok(Self::Zlib),
            "deflate-raw" => Ok(Self::Deflate),
            _ => bail!("unknown compression algorithm `{}`", s),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CompressionLevel(pub Compression);

impl CompressionLevel {
    pub fn from_attrs(attrs: &crate::pipelines::Attrs) -> anyhow::Result<Self> {
        attrs
            .get("data-compression-level")
            .map(|level| level.parse())
            .transpose()
            .map(|level| level.unwrap_or_default())
    }
}

impl FromStr for CompressionLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
        let level = match s {
            "0" => Compression::new(0),
            "1" => Compression::new(1),
            "2" => Compression::new(2),
            "3" => Compression::new(3),
            "4" => Compression::new(4),
            "5" => Compression::new(5),
            "6" => Compression::new(6),
            "7" => Compression::new(7),
            "8" => Compression::new(8),
            "9" => Compression::new(9),
            "default" => Compression::default(),
            "fast" => Compression::fast(),
            "best" => Compression::best(),
            _ => bail!("unknown compression level `{}`", s),
        };
        Ok(Self(level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::bufread::{DeflateDecoder, GzDecoder, ZlibDecoder};

    #[rstest::rstest]
    #[case(None, None, CompressionAlgorithm::None, Compression::default())]
    #[case(Some("gzip"), None, CompressionAlgorithm::Gzip, Compression::default())]
    #[case(
        Some("gzip"),
        Some("best"),
        CompressionAlgorithm::Gzip,
        Compression::best()
    )]
    #[case(None, Some("best"), CompressionAlgorithm::None, Compression::best())]
    #[case(
        Some("none"),
        Some("best"),
        CompressionAlgorithm::None,
        Compression::best()
    )]
    fn compression_attribute_defaults(
        #[case] algorithm: Option<&str>,
        #[case] level: Option<&str>,
        #[case] expected_algorithm: CompressionAlgorithm,
        #[case] expected_level: Compression,
    ) -> anyhow::Result<()> {
        let mut attrs = crate::pipelines::Attrs::new();
        if let Some(algorithm) = algorithm {
            attrs.insert("data-compression-algorithm".into(), algorithm.into());
        }
        if let Some(level) = level {
            attrs.insert("data-compression-level".into(), level.into());
        }
        assert_eq!(
            CompressionAlgorithm::from_attrs(&attrs)?,
            expected_algorithm
        );
        assert_eq!(
            CompressionLevel::from_attrs(&attrs)?,
            CompressionLevel(expected_level)
        );
        Ok(())
    }

    #[test]
    fn no_compression_passes_through() -> anyhow::Result<()> {
        assert_eq!(CompressionAlgorithm::default(), CompressionAlgorithm::None);
        assert_eq!(
            "none".parse::<CompressionAlgorithm>()?,
            CompressionAlgorithm::None
        );
        assert_eq!(CompressionAlgorithm::None.to_string(), "none");
        let input = b"uncompressed wasm";
        let mut output = Vec::new();
        CompressionAlgorithm::None
            .encoder(&input[..], Compression::default())
            .read_to_end(&mut output)?;
        assert_eq!(output, input);
        Ok(())
    }

    #[rstest::rstest]
    #[case("0", Compression::new(0))]
    #[case("1", Compression::new(1))]
    #[case("2", Compression::new(2))]
    #[case("3", Compression::new(3))]
    #[case("4", Compression::new(4))]
    #[case("5", Compression::new(5))]
    #[case("6", Compression::new(6))]
    #[case("7", Compression::new(7))]
    #[case("8", Compression::new(8))]
    #[case("9", Compression::new(9))]
    #[case("default", Compression::default())]
    #[case("fast", Compression::fast())]
    #[case("best", Compression::best())]
    fn compression_levels(
        #[case] input: &str,
        #[case] expected: Compression,
    ) -> anyhow::Result<()> {
        assert_eq!(
            input.parse::<CompressionLevel>()?,
            CompressionLevel(expected)
        );
        Ok(())
    }

    #[rstest::rstest]
    fn invalid_compression_levels(#[values("", "none", "10", "-1", "invalid")] input: &str) {
        assert!(input.parse::<CompressionLevel>().is_err());
    }

    #[rstest::rstest]
    fn algorithms_round_trip_even_at_level_zero(
        #[values("gzip", "deflate", "deflate-raw")] name: &str,
        #[values(Compression::new(0), Compression::default())] level: Compression,
    ) -> anyhow::Result<()> {
        let input = b"wasm compression round trip";
        let algorithm: CompressionAlgorithm = name.parse()?;
        assert_eq!(algorithm.to_string(), name);
        assert_eq!(
            name.to_uppercase().parse::<CompressionAlgorithm>()?,
            algorithm
        );
        let mut compressed = Vec::new();
        algorithm
            .encoder(&input[..], level)
            .read_to_end(&mut compressed)?;
        let mut decoder: Box<dyn Read> = match algorithm {
            CompressionAlgorithm::None => Box::new(&compressed[..]),
            CompressionAlgorithm::Gzip => Box::new(GzDecoder::new(&compressed[..])),
            CompressionAlgorithm::Zlib => Box::new(ZlibDecoder::new(&compressed[..])),
            CompressionAlgorithm::Deflate => Box::new(DeflateDecoder::new(&compressed[..])),
        };
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded)?;
        assert_eq!(decoded, input);
        assert!("invalid".parse::<CompressionAlgorithm>().is_err());
        Ok(())
    }
}
