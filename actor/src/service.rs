use async_trait::async_trait;
use futures_core::Stream;
use futures_util::stream::Empty;

use crate::{runtime::Runtime, Context, Envelope, Metadata};

/// [`Empty`] stream of [`Envelope`] for [`Context::new`] when there is no extra envelope source.
pub type EmptyStream<S> = Empty<Envelope<S>>;

/// A service is an running like thread
#[async_trait]
pub trait RuntimedService: Send + Sized + 'static {
    /// Extra envelope stream merged with the internal mailbox (see [`Context::with_stream`]).
    type Stream: Stream<Item = Envelope<Self>> + Unpin + Send;

    type Error: Send;

    type Runtime: Runtime;

    fn metadata(&self) -> Metadata<'_>;

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
