use async_trait::async_trait;

use crate::{
    Context, Message, ReplyHandle, RoutedTopic, Runtime, RuntimedHandler, RuntimedService, Topic,
};

/// Type-erased mailbox item for service `S`: a typed message dispatch or a topic operation.
///
/// Usually constructed with [`Envelope::new`], [`Envelope::new_with_result_channel`],
/// [`Envelope::new_subscribe_topic`], or [`Envelope::new_publish_topic`], then handled
/// by [`Envelope::handle`] inside the service run loop.
pub enum Envelope<S, R> {
    Message(Box<dyn EnvelopProxy<S, R> + Send>),
    StopService,
}

impl<S, R> Envelope<S, R> {
    /// Wrap a message for fire-and-forget delivery (no result sent to a caller).
    pub fn new<M>(message: M) -> Self
    where
        M: Message + Send + 'static,
        S: RuntimedHandler<M, R> + Send,
        M::Result: Send,
        R: Runtime,
    {
        Self::Message(Box::new(EnvelopWithMessage::new(message, None)))
    }

    /// Wrap a message and optionally send the handler's return value on `result_channel`.
    ///
    /// When `result_channel` is `None`, behaves like [`Self::new`].
    pub fn new_with_result_channel<M>(
        message: M,
        result_channel: Option<R::OneshotSender<M::Result>>,
    ) -> Self
    where
        S: RuntimedHandler<M, R> + Send,
        M: Message + Send + 'static,
        M::Result: Send,
        R: Runtime,
    {
        Self::Message(Box::new(EnvelopWithMessage::new(message, result_channel)))
    }

    /// Register a one-shot subscriber for a specific topic value.
    pub fn new_subscribe_topic<T>(topic: T, result_channel: R::OneshotSender<T::Item>) -> Self
    where
        S: RuntimedService<R> + Send,
        T: Topic + RoutedTopic<S, R>,
        R: Runtime,
    {
        Self::Message(Box::new(SubscribeTopicEnvelope::<T, R>::new(
            topic,
            result_channel,
        )))
    }

    /// Publish one item to a specific topic value.
    pub fn new_publish_topic<T>(topic: T, item: T::Item) -> Self
    where
        S: RuntimedService<R> + Send,
        T: Topic + RoutedTopic<S, R>,
        R: Runtime,
    {
        Self::Message(Box::new(PublishTopicEnvelope::<T>::new(topic, item)))
    }

    /// Register a subscriber waiting for all future publications.
    pub fn new_subscribe_all_topic<T>(topic: T, result_channel: R::UnboundedSender<T::Item>) -> Self
    where
        S: RuntimedService<R> + Send,
        T: Topic + RoutedTopic<S, R>,
        R: Runtime,
    {
        Self::Message(Box::new(SubscribeAllTopicEnvelope::<T, R>::new(
            topic,
            result_channel,
        )))
    }

    // /// Create an Envelope from a boxed EnvelopWithMessage without re-boxing
    // ///
    // /// This avoids an extra allocation when forwarding messages from Address to ServiceAddress.
    // /// The Box<EnvelopWithMessage<M>> is converted to Box<dyn EnvelopProxy<S> + Send> through
    // /// type erasure, which doesn't require re-allocation since EnvelopWithMessage<M> already
    // /// implements EnvelopProxy<S>.
    // pub(crate) fn from_boxed<M>(boxed: Box<EnvelopWithMessage<M>>) -> Self
    // where
    //     S: RuntimedHandler<M, R> + Send,
    //     M: Message + Send + 'static,
    //     M::Result: Send,
    //     R: Runtime,
    // {
    //     // Convert Box<EnvelopWithMessage<M>> to Box<dyn EnvelopProxy<S> + Send>
    //     // This is a type erasure that doesn't require re-allocation.
    //     // Rust automatically coerces Box<ConcreteType> to Box<dyn Trait> when ConcreteType implements Trait.
    //     Self(boxed)
    // }
}

impl<S, R> Envelope<S, R>
where
    S: RuntimedService<R> + Send,
    R: Runtime,
{
    /// Dispatch this envelope: run the message handler or apply the topic subscribe/publish.
    pub async fn handle(self, svc: &mut S, ctx: &mut Context<S, S::Stream, R>) {
        match self {
            Self::Message(message) => message.handle(svc, ctx).await,
            Self::StopService => ctx.stop(),
        }
    }
}

#[async_trait]
pub trait EnvelopProxy<S: RuntimedService<R>, R: Runtime> {
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S, S::Stream, R>);
}

pub(crate) struct EnvelopWithMessage<M, R>
where
    M: Message,
    R: Runtime,
{
    message: M,
    result_channel: Option<R::OneshotSender<M::Result>>,
}

impl<M, R> EnvelopWithMessage<M, R>
where
    M: Message,
    R: Runtime,
{
    pub(crate) fn new(message: M, result_channel: Option<R::OneshotSender<M::Result>>) -> Self {
        Self {
            message,
            result_channel,
        }
    }
}

#[async_trait]
impl<S, M, R> EnvelopProxy<S, R> for EnvelopWithMessage<M, R>
where
    M: Message + Send + 'static,
    S: RuntimedHandler<M, R> + Send,
    M::Result: Send,
    R: Runtime,
{
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S, S::Stream, R>) {
        let message = self.message;
        let result_channel = self.result_channel;

        let handle = ReplyHandle::new(result_channel);
        <S as RuntimedHandler<M, R>>::handle_preferred(svc, message, ctx, handle).await;
    }
}

pub(crate) struct SubscribeTopicEnvelope<T, R>
where
    T: Topic,
    R: Runtime,
{
    topic: T,
    result_channel: R::OneshotSender<T::Item>,
}

impl<T, R> SubscribeTopicEnvelope<T, R>
where
    T: Topic,
    R: Runtime,
{
    pub(crate) fn new(topic: T, result_channel: R::OneshotSender<T::Item>) -> Self {
        Self {
            topic,
            result_channel,
        }
    }
}

#[async_trait]
impl<S, T, R> EnvelopProxy<S, R> for SubscribeTopicEnvelope<T, R>
where
    S: RuntimedService<R> + Send,
    T: Topic + RoutedTopic<S, R>,
    R: Runtime,
{
    async fn handle(self: Box<Self>, svc: &mut S, _ctx: &mut Context<S, S::Stream, R>) {
        let Self {
            topic,
            result_channel,
        } = *self;
        T::endpoint(svc).subscribe(topic, result_channel);
    }
}

pub(crate) struct PublishTopicEnvelope<T>
where
    T: Topic,
{
    topic: T,
    item: T::Item,
}

impl<T> PublishTopicEnvelope<T>
where
    T: Topic,
{
    pub(crate) fn new(topic: T, item: T::Item) -> Self {
        Self { topic, item }
    }
}

#[async_trait]
impl<S, T, R> EnvelopProxy<S, R> for PublishTopicEnvelope<T>
where
    S: RuntimedService<R> + Send,
    T: Topic + RoutedTopic<S, R>,
    R: Runtime,
{
    async fn handle(self: Box<Self>, svc: &mut S, _ctx: &mut Context<S, S::Stream, R>) {
        let Self { topic, item } = *self;
        T::endpoint(svc).publish(&topic, item);
    }
}

pub(crate) struct SubscribeAllTopicEnvelope<T, R>
where
    T: Topic,
    R: Runtime,
{
    topic: T,
    result_channel: R::UnboundedSender<T::Item>,
}

impl<T, R> SubscribeAllTopicEnvelope<T, R>
where
    T: Topic,
    R: Runtime,
{
    pub(crate) fn new(topic: T, result_channel: R::UnboundedSender<T::Item>) -> Self {
        Self {
            topic,
            result_channel,
        }
    }
}

#[async_trait]
impl<S, T, R> EnvelopProxy<S, R> for SubscribeAllTopicEnvelope<T, R>
where
    S: RuntimedService<R> + Send,
    T: Topic + RoutedTopic<S, R>,
    R: Runtime,
{
    async fn handle(self: Box<Self>, svc: &mut S, _ctx: &mut Context<S, S::Stream, R>) {
        let Self {
            topic,
            result_channel,
        } = *self;
        T::endpoint(svc).subscribe_all(topic, result_channel);
    }
}
