use clap::ValueEnum;

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, serde::Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AddressFamily {
    Ipv4,
    #[default]
    Ipv6,
}
