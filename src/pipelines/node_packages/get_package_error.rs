use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Error)]
pub enum GetPackageError {
    #[error("failed to send `GetPackage` request: {0}")]
    Send(String),
    #[error("failed to receive `GetPackage` response: {0}")]
    Receive(String),
    #[error("node package not found")]
    NotFound,
}
