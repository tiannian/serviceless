use crate::{Context, Envelope, Metadata, RuntimedService};
use futures_core::Stream;

use async_trait::async_trait;

#[async_trait]
pub trait Service: Send + Sized + 'static {
    type Stream: Stream<Item = Envelope<Self>> + Unpin + Send;

    type Error: Send;

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

#[async_trait]
impl<T> RuntimedService for T
where
    T: Service,
{
    type Stream = T::Stream;

    type Error = T::Error;

    type Runtime = crate::runtime_impl::tokio::TokioRuntime;

    fn metadata(&self) -> Metadata<'_> {
        self.metadata()
    }

    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        self.started(_ctx).await
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        self.stopped(_ctx).await
    }
}
