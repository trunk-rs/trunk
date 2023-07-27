use tokio::task::JoinHandle;

use super::output::Output;
use crate::util::Result;

/// A type that is used as an input to an asset pipeline.
#[derive(Debug, Clone)]
pub struct AssetInput {}

/// If an input is accepted, then it will return Ok(()), otherwise, the input is returned with
/// Err(input).
///
/// Unlike other errors, this error is not fatal as it can be passed to the next pipeline.
pub type InputPushResult = std::result::Result<(), AssetInput>;

/// A type that can be used as an asset pipeline.
pub trait Asset {
    type Output: Output;

    fn try_push_input(&mut self, input: AssetInput) -> Result<InputPushResult> {
        Ok(Err(input))
    }

    /// Spawns the pipeline for this asset type.
    fn spawn(self) -> JoinHandle<Result<Self::Output>>;
}
