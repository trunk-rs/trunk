use anyhow::bail;
use flate2::{
    Compression,
    bufread::{DeflateEncoder, GzEncoder, ZlibEncoder},
};
use std::{
    io::{BufRead, Read},
    str::FromStr,
};

#[derive(PartialEq, Eq, Debug, Default, strum::Display)]
pub enum CompressionAlgorithm {
    #[default]
    #[strum(serialize = "gzip")]
    Gzip,
    #[strum(serialize = "deflate")]
    Zlib,
    #[strum(serialize = "deflate-raw")]
    Deflate,
}

impl CompressionAlgorithm {
    pub fn encoder<'a, R: BufRead + Send + 'a>(
        &self,
        reader: R,
        level: Compression,
    ) -> Box<dyn Read + Send + 'a> {
        match self {
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
            "gzip" => Ok(Self::Gzip),
            "deflate" => Ok(Self::Zlib),
            "deflate-raw" => Ok(Self::Deflate),
            _ => bail!("unknown compression algorithm `{}`", s),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompressionLevel {
    None,
    Level(Compression),
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::Level(Compression::default())
    }
}

impl FromStr for CompressionLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
        let level = match s {
            "none" => return Ok(Self::None),
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
        Ok(Self::Level(level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::bufread::{DeflateDecoder, GzDecoder, ZlibDecoder};

    #[test]
    fn compression_levels() -> anyhow::Result<()> {
        assert_eq!("none".parse::<CompressionLevel>()?, CompressionLevel::None);
        for level in 0..=9 {
            assert_eq!(
                level.to_string().parse::<CompressionLevel>()?,
                CompressionLevel::Level(Compression::new(level))
            );
        }
        for (name, level) in [
            ("default", Compression::default()),
            ("fast", Compression::fast()),
            ("best", Compression::best()),
        ] {
            assert_eq!(
                name.parse::<CompressionLevel>()?,
                CompressionLevel::Level(level)
            );
        }
        for invalid in ["", "10", "-1", "invalid"] {
            assert!(invalid.parse::<CompressionLevel>().is_err());
        }
        Ok(())
    }

    #[test]
    fn algorithms_round_trip_even_at_level_zero() -> anyhow::Result<()> {
        let input = b"wasm compression round trip";
        for name in ["gzip", "deflate", "deflate-raw"] {
            let algorithm: CompressionAlgorithm = name.parse()?;
            assert_eq!(algorithm.to_string(), name);
            assert_eq!(
                name.to_uppercase().parse::<CompressionAlgorithm>()?,
                algorithm
            );
            for level in [Compression::new(0), Compression::default()] {
                let mut compressed = Vec::new();
                algorithm
                    .encoder(&input[..], level)
                    .read_to_end(&mut compressed)?;
                let mut decoder: Box<dyn Read> = match algorithm {
                    CompressionAlgorithm::Gzip => Box::new(GzDecoder::new(&compressed[..])),
                    CompressionAlgorithm::Zlib => Box::new(ZlibDecoder::new(&compressed[..])),
                    CompressionAlgorithm::Deflate => Box::new(DeflateDecoder::new(&compressed[..])),
                };
                let mut decoded = Vec::new();
                decoder.read_to_end(&mut decoded)?;
                assert_eq!(decoded, input);
            }
        }
        assert!("invalid".parse::<CompressionAlgorithm>().is_err());
        Ok(())
    }
}
