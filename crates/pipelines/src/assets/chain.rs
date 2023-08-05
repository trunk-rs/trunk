use async_trait::async_trait;
use futures_util::future::BoxFuture;
use futures_util::stream::{self, MapOk, Select};
use futures_util::{FutureExt, TryFutureExt, TryStreamExt};
use tokio::task::JoinHandle;
use trunk_util::ErrorReason;

use super::{Asset, Output};
use crate::util::Result;

/// Chains 2 pipelines together
///
/// This type tries the first pipeline, if fails, tries the second one.
#[derive(Debug)]
pub struct Chain<A, B> {
    pub(crate) first: A,
    pub(crate) second: B,
}

#[async_trait]
impl<A, B> Asset for Chain<A, B>
where
    A: Asset + Send + Sync + 'static,
    B: Asset + Send + Sync + 'static,
{
    type Output = ChainOutput<A, B>;
    type OutputStream = Select<
        MapOk<A::OutputStream, fn(A::Output) -> ChainOutput<A, B>>,
        MapOk<B::OutputStream, fn(B::Output) -> ChainOutput<A, B>>,
    >;
    type RunOnceFuture<'a> = BoxFuture<'a, Result<Self::Output>>;

    fn run_once(&self, input: super::AssetInput) -> Self::RunOnceFuture<'_> {
        self.first
            .run_once(input)
            .map_ok(|m| ChainOutput::First(m))
            .or_else(move |e| async move {
                match *e.reason {
                    ErrorReason::AssetNotMatched { input } => {
                        self.second
                            .run_once(input)
                            .map_ok(|m| ChainOutput::Second(m))
                            .await
                    }
                    _ => Err(e),
                }
            })
            .boxed()
    }

    fn outputs(self) -> Self::OutputStream {
        stream::select(
            self.first.outputs().map_ok(ChainOutput::First),
            self.second.outputs().map_ok(ChainOutput::Second),
        )
    }

    async fn try_push_input(&mut self, input: super::AssetInput) -> Result<()> {
        match self.first.try_push_input(input).await {
            Ok(m) => Ok(m),
            Err(e) => match *e.reason {
                ErrorReason::AssetNotMatched { input } => self.second.try_push_input(input).await,
                _ => Err(e),
            },
        }
    }

    fn spawn(self) -> JoinHandle<Result<Self::Output>> {
        todo!()
    }
}

/// The output of a chained output.
#[derive(Debug)]
pub enum ChainOutput<A, B>
where
    A: Asset + 'static,
    B: Asset + 'static,
{
    First(A::Output),
    Second(B::Output),
}

impl<A, B> Output for ChainOutput<A, B>
where
    A: Asset + 'static,
    B: Asset + 'static,
{
    fn finalize<'life0, 'async_trait>(
        self,
        dom: &'life0 mut nipper::Document,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = trunk_util::Result<()>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        match self {
            Self::First(l) => l.finalize(dom).boxed(),
            Self::Second(r) => r.finalize(dom).boxed(),
        }
    }
}
