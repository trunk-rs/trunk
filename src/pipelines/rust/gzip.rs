use std::{ops::Deref, str::FromStr};

use anyhow::bail;
use flate2::Compression;

#[derive(PartialEq, Eq)]
pub struct GzipLevel(Compression);

impl GzipLevel {
    pub const OFF: Self = Self(Compression::new(0));
}

impl FromStr for GzipLevel {
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

impl AsRef<Compression> for GzipLevel {
    fn as_ref(&self) -> &Compression {
        &self.0
    }
}

impl Deref for GzipLevel {
    type Target = Compression;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for GzipLevel {
    fn default() -> Self {
        Self(Compression::default())
    }
}
