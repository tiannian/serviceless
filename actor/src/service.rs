use async_trait::async_trait;
use futures_core::Stream;
use std::future::Future;

use crate::{Context, Envelope, ServiceAddress};

/// A service is an running like thread
#[async_trait]
pub trait Service: Send + Sized + 'static {
    type Stream: Stream<Item = Envelope<Self>> + Unpin + Send;

    /// Start a service with the given context
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    fn start_by_context(
        self,
        ctx: Context<Self, Self::Stream>,
    ) -> (ServiceAddress<Self>, impl Future<Output = ()> + Send) {
        ctx.run(self)
    }

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Context<Self, Self::Stream>) {}

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Context<Self, Self::Stream>) {}
}
