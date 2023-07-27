use std::str::FromStr;

use crate::util::{Error, ErrorReason, Result};

/// Describes how the rust application is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustAppType {
    /// Used as the main application.
    Main,
    /// Used as a web worker.
    Worker,
}

impl FromStr for RustAppType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "main" => Ok(RustAppType::Main),
            "worker" => Ok(RustAppType::Worker),
            _ => Err(ErrorReason::RustUnknownAppType {
                type_str: s.to_string(),
            }
            .into_error()),
        }
    }
}
