use std::fmt::{Display, Formatter};

/// Cross origin setting
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum CrossOrigin {
    #[default]
    Anonymous,
    UseCredentials,
}

impl CrossOrigin {
    pub fn from_str(s: &str) -> anyhow::Result<Self, CrossOriginParseError> {
        Ok(match s {
            "" | "anonymous" => CrossOrigin::Anonymous,
            "use-credentials" => CrossOrigin::UseCredentials,
            _ => return Err(CrossOriginParseError::InvalidValue),
        })
    }
}

impl Display for CrossOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anonymous => write!(f, "anonymous"),
            Self::UseCredentials => write!(f, "use-credentials"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CrossOriginParseError {
    #[error("invalid value")]
    InvalidValue,
}
