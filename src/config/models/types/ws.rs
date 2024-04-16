use clap::ValueEnum;
use serde::Deserialize;
use std::fmt::{Display, Formatter};

/// WebSocket protocol
#[derive(Clone, Copy, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum WsProtocol {
    Wss,
    Ws,
}

impl Display for WsProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WsProtocol::Wss => "wss",
                WsProtocol::Ws => "ws",
            }
        )
    }
}
