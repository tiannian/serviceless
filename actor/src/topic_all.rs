use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use futures_util::StreamExt;

use crate::{
    runtime::{AsyncUnboundedReceiver, Runtime, UnboundedReceiverBase},
    Topic,
};

pub struct RuntimedTopicAllHandle<T: Topic, R>
where
    R: Runtime,
{
    receiver: R::AsyncUnboundedReceiver<T::Item>,
}

impl<T, R> RuntimedTopicAllHandle<T, R>
where
    T: Topic,
    R: Runtime,
{
    pub(crate) fn new(receiver: R::AsyncUnboundedReceiver<T::Item>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Option<T::Item> {
        self.receiver.recv().await.ok()
    }

    pub fn close(&mut self) {
        self.receiver.close();
    }
}

impl<T, R> Stream for RuntimedTopicAllHandle<T, R>
where
    T: Topic,
    R: Runtime,
{
    type Item = T::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(cx)
    }
}
