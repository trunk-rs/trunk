use clap::ValueEnum;
use schemars::JsonSchema;

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, serde::Deserialize, ValueEnum, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AddressFamily {
    Ipv4,
    #[default]
    Ipv6,
}
