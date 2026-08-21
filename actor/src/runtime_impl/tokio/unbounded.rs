use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::runtime::{AsyncUnboundedReceiver, InnerOp, UnboundedReceiverBase, UnboundedSender};
use async_trait::async_trait;
use futures_core::Stream;
use tokio::sync::mpsc;

use super::Error;

pub struct TokioUnboundedSender<T> {
    pub(crate) sender: mpsc::UnboundedSender<T>,
}

impl<T> Clone for TokioUnboundedSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T> InnerOp for TokioUnboundedSender<T> {
    type InnerType = mpsc::UnboundedSender<T>;

    fn into_inner(self) -> Self::InnerType {
        self.sender
    }

    fn inner(&self) -> &Self::InnerType {
        &self.sender
    }

    fn inner_mut(&mut self) -> &mut Self::InnerType {
        &mut self.sender
    }
}

impl<T> UnboundedSender<T> for TokioUnboundedSender<T>
where
    T: Send,
{
    type Error = Error;

    fn send(&self, item: T) -> Result<(), Self::Error> {
        self.sender
            .send(item)
            .map_err(|_| Error::UnboundedChannelClosed)
    }

    fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

pub struct TokioUnboundedReceiver<T> {
    pub(crate) receiver: mpsc::UnboundedReceiver<T>,
}

impl<T> InnerOp for TokioUnboundedReceiver<T> {
    type InnerType = mpsc::UnboundedReceiver<T>;

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

impl<T> Stream for TokioUnboundedReceiver<T>
where
    T: Send,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self;

        Pin::new(&mut this.receiver).poll_recv(cx)
    }
}

impl<T> UnboundedReceiverBase for TokioUnboundedReceiver<T>
where
    T: Send,
{
    type Error = Error;

    fn close(&mut self) {
        self.receiver.close();
    }

    fn len(&self) -> usize {
        self.receiver.len()
    }
}

#[async_trait]
impl<T> AsyncUnboundedReceiver<T> for TokioUnboundedReceiver<T>
where
    T: Send,
{
    async fn recv(&mut self) -> Result<T, Self::Error> {
        self.receiver
            .recv()
            .await
            .ok_or(Error::UnboundedChannelClosed)
    }
}
