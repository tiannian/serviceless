use async_trait::async_trait;
use serviceless_core::{Handler, Message, Service};
use tokio::sync::oneshot;

pub(crate) struct Envelope<S>(Box<dyn EnvelopProxy<S> + Send>);

impl<S> Envelope<S>
where
    S: Service + Send,
    S::Runtime: Send,
{
    pub fn new<M>(message: M, result_channel: Option<oneshot::Sender<M::Result>>) -> Self
    where
        S: Handler<M>,
        M: Message + Send + 'static,
        M::Result: Send,
    {
        Self(Box::new(EnvelopWithMessage {
            message: Some(message),
            result_channel,
        }))
    }
}

#[async_trait]
impl<S> EnvelopProxy<S> for Envelope<S>
where
    S: Service + Send,
    S::Runtime: Send,
{
    async fn handle(&mut self, svc: &mut S, rt: &mut S::Runtime) {
        let r = &mut self.0;

        r.handle(svc, rt).await
    }
}

#[async_trait]
pub(crate) trait EnvelopProxy<S>
where
    S: Service,
{
    async fn handle(&mut self, svc: &mut S, rt: &mut S::Runtime);
}

pub(crate) struct EnvelopWithMessage<M>
where
    M: Message,
{
    message: Option<M>,
    result_channel: Option<oneshot::Sender<M::Result>>,
}

#[async_trait]
impl<S, M> EnvelopProxy<S> for EnvelopWithMessage<M>
where
    M: Message + Send,
    M::Result: Send,
    S: Service + Handler<M> + Send,
    S::Runtime: Send,
{
    async fn handle(&mut self, svc: &mut S, ctx: &mut S::Runtime) {
        let message = self.message.take();
        let result_channel = self.result_channel.take();

        if let (Some(message), Some(rc)) = (message, result_channel) {
            let res = <S as Handler<M>>::handler(svc, message, ctx).await;

            if rc.send(res).is_err() {
                log::error!("Channel Closed");
            }
        }
    }
}
