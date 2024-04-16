use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ConfigDuration(pub Duration);

impl<'de> Deserialize<'de> for ConfigDuration {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(humantime_serde::deserialize(deserializer)?))
    }
}

impl FromStr for ConfigDuration {
    type Err = humantime::DurationError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(humantime::Duration::from_str(s)?.into()))
    }
}
