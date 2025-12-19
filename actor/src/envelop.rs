use async_trait::async_trait;
use service_channel::oneshot;

use crate::{Context, Handler, Message};

pub(crate) struct Envelope<S>(Box<dyn EnvelopProxy<S> + Send>);

impl<S> Envelope<S> {
    pub fn new<M>(message: M, result_channel: Option<oneshot::Sender<M::Result>>) -> Self
    where
        S: Handler<M> + Send,
        M: Message + Send + 'static,
        M::Result: Send,
    {
        Self(Box::new(EnvelopWithMessage::new(message, result_channel)))
    }

    /// Create an Envelope from a boxed EnvelopWithMessage without re-boxing
    ///
    /// This avoids an extra allocation when forwarding messages from Address to ServiceAddress.
    /// The Box<EnvelopWithMessage<M>> is converted to Box<dyn EnvelopProxy<S> + Send> through
    /// type erasure, which doesn't require re-allocation since EnvelopWithMessage<M> already
    /// implements EnvelopProxy<S>.
    pub fn from_boxed<M>(boxed: Box<EnvelopWithMessage<M>>) -> Self
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
    S: Send,
{
    pub async fn handle(self, svc: &mut S, ctx: &mut Context<S>) {
        self.0.handle(svc, ctx).await
    }
}

#[async_trait]
pub(crate) trait EnvelopProxy<S> {
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S>);
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
    async fn handle(mut self: Box<Self>, svc: &mut S, ctx: &mut Context<S>) {
        let message = self.message;
        let result_channel = self.result_channel;

            let res = <S as Handler<M>>::handler(svc, message, ctx).await;

            if let Some(rc) = result_channel {
                if rc.send(res).is_err() {
                    log::warn!("Channel Closed");
                }
            }
    }
}
