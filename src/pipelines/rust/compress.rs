use std::{
    io::{BufRead, Read},
    ops::Deref,
    str::FromStr,
};

use anyhow::bail;
use flate2::{
    bufread::{DeflateEncoder, GzEncoder, ZlibEncoder},
    Compression,
};

#[derive(PartialEq, Eq, Debug, Default)]
pub enum CompressionAlgorithm {
    #[default]
    Gzip,
    Zlib,
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

    pub fn as_string(&self) -> String {
        match self {
            Self::Gzip => "gzip".to_string(),
            Self::Zlib => "deflate".to_string(),
            Self::Deflate => "deflate-raw".to_string(),
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

#[derive(Default, PartialEq, Eq)]
pub struct CompressionLevel(Compression);

impl CompressionLevel {
    pub const OFF: Self = Self(Compression::new(0));
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
            _ => bail!("unknown gzip level `{}`", s),
        };
        Ok(Self(level))
    }
}

impl AsRef<Compression> for CompressionLevel {
    fn as_ref(&self) -> &Compression {
        &self.0
    }
}

impl Deref for CompressionLevel {
    type Target = Compression;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}