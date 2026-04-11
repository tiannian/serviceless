use async_trait::async_trait;
use service_channel::oneshot;

use crate::{Context, Handler, Message, RoutedTopic, Service, Topic};

/// Type-erased mailbox item for service `S`: a typed message dispatch or a topic operation.
///
/// Usually constructed with [`Envelope::new`], [`Envelope::new_with_result_channel`],
/// [`Envelope::new_subscribe_topic`], or [`Envelope::new_publish_topic`], then handled
/// by [`Envelope::handle`] inside the service run loop.
pub struct Envelope<S>(Box<dyn EnvelopProxy<S> + Send>);

impl<S> Envelope<S> {
    /// Wrap a message for fire-and-forget delivery (no result sent to a caller).
    pub fn new<M>(message: M) -> Self
    where
        M: Message + Send + 'static,
        S: Handler<M> + Send,
        M::Result: Send,
    {
        Self(Box::new(EnvelopWithMessage::new(message, None)))
    }

    /// Wrap a message and optionally send the handler's return value on `result_channel`.
    ///
    /// When `result_channel` is `None`, behaves like [`Self::new`].
    pub fn new_with_result_channel<M>(
        message: M,
        result_channel: Option<oneshot::Sender<M::Result>>,
    ) -> Self
    where
        S: Handler<M> + Send,
        M: Message + Send + 'static,
        M::Result: Send,
    {
        Self(Box::new(EnvelopWithMessage::new(message, result_channel)))
    }

    /// Register a one-shot subscriber for topic T.
    pub fn new_subscribe_topic<T>(result_channel: oneshot::Sender<T::Item>) -> Self
    where
        S: Service + Send,
        T: Topic + RoutedTopic<S>,
    {
        Self(Box::new(SubscribeTopicEnvelope::<T>::new(result_channel)))
    }

    /// Publish one item to topic T.
    pub fn new_publish_topic<T>(item: T::Item) -> Self
    where
        S: Service + Send,
        T: Topic + RoutedTopic<S>,
    {
        Self(Box::new(PublishTopicEnvelope::<T>::new(item)))
    }

    /// Create an Envelope from a boxed EnvelopWithMessage without re-boxing
    ///
    /// This avoids an extra allocation when forwarding messages from Address to ServiceAddress.
    /// The Box<EnvelopWithMessage<M>> is converted to Box<dyn EnvelopProxy<S> + Send> through
    /// type erasure, which doesn't require re-allocation since EnvelopWithMessage<M> already
    /// implements EnvelopProxy<S>.
    pub(crate) fn from_boxed<M>(boxed: Box<EnvelopWithMessage<M>>) -> Self
    where
        S: Handler<M> + Send,
        M: Message + Send + 'static,
        M::Result: Send,
    {
        // Convert Box<EnvelopWithMessage<M>> to Box<dyn EnvelopProxy<S> + Send>
        // This is a type erasure that doesn't require re-allocation.
        // Rust automatically coerces Box<ConcreteType> to Box<dyn Trait> when ConcreteType implements Trait.
        Self(boxed)
    }
}

impl<S> Envelope<S>
where
    S: Service + Send,
{
    /// Dispatch this envelope: run the message handler or apply the topic subscribe/publish.
    pub async fn handle(self, svc: &mut S, ctx: &mut Context<S, S::Stream>) {
        self.0.handle(svc, ctx).await
    }
}

#[async_trait]
pub(crate) trait EnvelopProxy<S: Service> {
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S, S::Stream>);
}

pub(crate) struct EnvelopWithMessage<M>
where
    M: Message,
{
    message: M,
    result_channel: Option<oneshot::Sender<M::Result>>,
}

impl<M> EnvelopWithMessage<M>
where
    M: Message,
{
    pub(crate) fn new(message: M, result_channel: Option<oneshot::Sender<M::Result>>) -> Self {
        Self {
            message,
            result_channel,
        }
    }
}

#[async_trait]
impl<S, M> EnvelopProxy<S> for EnvelopWithMessage<M>
where
    M: Message + Send,
    S: Handler<M> + Send,
    M::Result: Send,
{
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S, S::Stream>) {
        let message = self.message;
        let result_channel = self.result_channel;

        let res = <S as Handler<M>>::handle(svc, message, ctx).await;

        if let Some(rc) = result_channel {
            if rc.send(res).is_err() {
                log::warn!("Channel Closed");
            }
        }
    }
}

pub(crate) struct SubscribeTopicEnvelope<T>
where
    T: Topic,
{
    result_channel: Option<oneshot::Sender<T::Item>>,
}

impl<T> SubscribeTopicEnvelope<T>
where
    T: Topic,
{
    pub(crate) fn new(result_channel: oneshot::Sender<T::Item>) -> Self {
        Self {
            result_channel: Some(result_channel),
        }
    }
}

#[async_trait]
impl<S, T> EnvelopProxy<S> for SubscribeTopicEnvelope<T>
where
    S: Service + Send,
    T: Topic + RoutedTopic<S>,
{
    async fn handle(mut self: Box<Self>, svc: &mut S, _ctx: &mut Context<S, S::Stream>) {
        if let Some(tx) = self.result_channel.take() {
            T::endpoint(svc).subscribe(tx);
        }
    }
}

pub(crate) struct PublishTopicEnvelope<T>
where
    T: Topic,
{
    item: Option<T::Item>,
}

impl<T> PublishTopicEnvelope<T>
where
    T: Topic,
{
    pub(crate) fn new(item: T::Item) -> Self {
        Self { item: Some(item) }
    }
}

#[async_trait]
impl<S, T> EnvelopProxy<S> for PublishTopicEnvelope<T>
where
    S: Service + Send,
    T: Topic + RoutedTopic<S>,
{
    async fn handle(mut self: Box<Self>, svc: &mut S, _ctx: &mut Context<S, S::Stream>) {
        if let Some(item) = self.item.take() {
            T::endpoint(svc).publish(item);
        }
    }
}
