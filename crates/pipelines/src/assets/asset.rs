use async_trait::async_trait;
use futures_util::Stream;
#[doc(inline)]
pub use trunk_util::AssetInput;

use super::chain::Chain;
use super::output::Output;
use crate::util::{ErrorReason, Result};

/// A type that can be used as an asset pipeline.
#[async_trait]
pub trait Asset {
    type Output: Output;
    type OutputStream: Stream<Item = Result<Self::Output>> + Send;

    /// Tries to push an input to this pipeline, rejects if it fails to parse.
    ///
    /// When an input is not accepted by current pipeline but could possibly be accepted in other
    /// pipelines, it should be rejected with `ErrorReason::AssetNotMatched`
    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        Err(ErrorReason::AssetNotMatched { input }.into_error())
    }

    /// Chains 2 Pipelines together.
    fn chain<Other>(self, other: Other) -> Chain<Self, Other>
    where
        Self: Sized,
    {
        Chain {
            first: self,
            second: other,
        }
    }

    /// Runs this pipeline once with an input
    async fn run_once(&self, input: AssetInput) -> Result<Self::Output>;

    /// Runs current pipeline with all previously accepted inputs.
    fn outputs(self) -> Self::OutputStream;

    /// Boxing an asset.
    ///
    /// This method is used to avoid stack overflow when many assets are being processed.
    fn boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}
