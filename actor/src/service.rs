use async_trait::async_trait;
use futures_core::Stream;
use futures_util::stream::Empty;
use std::future::Future;

use crate::{runtime::Runtime, Context, Envelope, Metadata, ServiceAddress};

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

    /// Start a service with the given context
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    fn start_by_context(
        self,
        ctx: Context<Self>,
    ) -> (
        ServiceAddress<Self>,
        impl Future<Output = Result<(), Self::Error>> + Send,
    ) {
        ctx.run(self)
    }

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
