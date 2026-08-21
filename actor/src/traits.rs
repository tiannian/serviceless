use async_trait::async_trait;
use futures_core::Stream;
use futures_util::stream::Empty;

use crate::{
    runtime::Runtime, Context, Envelope, Metadata, RuntimedReplyHandle, RuntimedTopicEndpoint,
};

/// [`Empty`] stream of [`Envelope`] for [`Context::new`] when there is no extra envelope source.
pub type EmptyStream<S> = Empty<Envelope<S>>;

/// A service is an running like thread.
#[async_trait]
pub trait RuntimedService: Send + Sized + 'static {
    /// Extra envelope stream merged with the internal mailbox (see [`Context::with_stream`]).
    type Stream: Stream<Item = Envelope<Self>> + Unpin + Send;

    type Runtime: Runtime;

    fn metadata(&self) -> Metadata<'_>;

    /// Hook for service started.
    async fn started(&mut self, _ctx: &mut Context<Self>) {}

    /// Hook for service stopped.
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {}

    fn sync_started(&mut self, _ctx: &mut Context<Self>) {}

    fn sync_stopped(&mut self, _ctx: &mut Context<Self>) {}
}

/// Handles a message on a service.
#[async_trait]
pub trait Handler<M>
where
    Self: RuntimedService + Sized,
    M: Message,
{
    /// Handle a message.
    async fn handle(&mut self, message: M, ctx: &mut Context<Self>) -> M::Result;

    /// Handle a message with a reply handle.
    async fn handle_preferred(
        &mut self,
        message: M,
        ctx: &mut Context<Self>,
        handle: RuntimedReplyHandle<M, <Self::Runtime as Runtime>::OneshotSender<M::Result>>,
    ) where
        M: Send + 'static,
        M::Result: Send,
    {
        let res = self.handle(message, ctx).await;
        let _ = handle.send(res);
    }
}

pub trait SyncHandler<M>
where
    Self: RuntimedService + Sized,
    M: Message,
{
    fn sync_handle(&mut self, message: M, ctx: &mut Context<Self>) -> M::Result;

    fn sync_handle_preferred(
        &mut self,
        message: M,
        ctx: &mut Context<Self>,
        handle: RuntimedReplyHandle<M, <Self::Runtime as Runtime>::OneshotSender<M::Result>>,
    ) where
        M: Send + 'static,
        M::Result: Send,
    {
        let res = self.sync_handle(message, ctx);
        let _ = handle.send(res);
    }
}

/// A message handled by a service.
pub trait Message {
    /// Result of the message.
    type Result;
}

/// A typed pub/sub topic.
pub trait Topic: Ord + Clone + Send + Unpin + 'static {
    type Item: Clone + Send + Unpin + 'static;
}

/// Binds a topic to a concrete endpoint field on a service.
///
/// This is the key piece that replaces Any/TypeId routing:
/// each topic knows where its endpoint lives on service S.
pub trait RoutedTopic<S>: Topic
where
    S: RuntimedService,
{
    /// Returns this topic's [`RuntimedTopicEndpoint`] on `service`.
    ///
    /// Implementations should consistently point at the same logical field on `S` so
    /// routing matches how the service stores topic state.
    fn endpoint(
        service: &mut S,
    ) -> &mut RuntimedTopicEndpoint<
        Self,
        <S::Runtime as Runtime>::OneshotSender<Self::Item>,
        <S::Runtime as Runtime>::AsyncUnboundedSender<Self::Item>,
    >
    where
        Self: Sized;
}
