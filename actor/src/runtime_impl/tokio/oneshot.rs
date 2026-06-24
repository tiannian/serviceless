use crate::runtime::{InnerOp, OneshotReceiver, OneshotSender};
use async_trait::async_trait;
use tokio::sync::oneshot;

use super::Error;

pub struct TokioOneshotSender<T> {
    pub(crate) sender: oneshot::Sender<T>,
}

impl<T> InnerOp for TokioOneshotSender<T> {
    type InnerType = oneshot::Sender<T>;

    fn inner(&self) -> &Self::InnerType {
        &self.sender
    }

    fn inner_mut(&mut self) -> &mut Self::InnerType {
        &mut self.sender
    }

    fn into_inner(self) -> Self::InnerType {
        self.sender
    }
}

impl<T> OneshotSender<T> for TokioOneshotSender<T>
where
    T: Send,
{
    type Error = Error;

    fn send(self, item: T) -> Result<(), Self::Error> {
        self.sender
            .send(item)
            .map_err(|_| Error::OneshotChannelClosed)
    }

    fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

pub struct TokioOneshotReceiver<T> {
    pub(crate) receiver: oneshot::Receiver<T>,
}

impl<T> InnerOp for TokioOneshotReceiver<T> {
    type InnerType = oneshot::Receiver<T>;

    fn into_inner(self) -> Self::InnerType {
        self.receiver
    }

    fn inner(&self) -> &Self::InnerType {
        &self.receiver
    }

    fn inner_mut(&mut self) -> &mut Self::InnerType {
        &mut self.receiver
    }
}

#[async_trait]
impl<T> OneshotReceiver<T> for TokioOneshotReceiver<T>
where
    T: Send,
{
    type Error = Error;

    async fn recv(mut self) -> Result<T, Self::Error> {
        self.receiver
            .try_recv()
            .map_err(|_| Error::OneshotChannelClosed)
    }

    fn close(&mut self) {
        self.receiver.close();
    }
}
