use async_trait::async_trait;
use nipper::Document;

use crate::util::Result;

/// A pipeline output.
#[async_trait]
pub trait Output {
    /// Finalise current output.
    async fn finalize(self, dom: &mut Document) -> Result<()>;
}
