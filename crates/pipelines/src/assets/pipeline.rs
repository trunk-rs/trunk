use tokio::task::JoinHandle;

use super::output::Output;
use crate::util::Result;

/// A type that can be used as a pipeline.
pub trait Pipeline {
    type Output: Output;

    /// Spawns the pipeline for this asset type.
    fn spawn(self) -> JoinHandle<Result<Self::Output>>;
}
