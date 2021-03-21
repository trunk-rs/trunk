#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidArgs(serde_cbor::Error),
    #[error("Failed to access {len} bytes at {ptr}")]
    InvalidMemorySlice { ptr: u32, len: u32 },
    #[error(transparent)]
    SerdeCbor(#[from] serde_cbor::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    #[cfg(feature = "runtime")]
    WasmerRuntime(#[from] wasmer_runtime::error::Error),
}
