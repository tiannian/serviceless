use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use futures_util::StreamExt;

use crate::{runtime::AsyncUnboundedReceiver, Topic};

pub struct RuntimedTopicAllHandle<T: Topic, R>
where
    R: AsyncUnboundedReceiver<T::Item>,
{
    receiver: R,
    marker: PhantomData<T>,
}

impl<T, R> RuntimedTopicAllHandle<T, R>
where
    T: Topic,
    R: AsyncUnboundedReceiver<T::Item>,
{
    pub(crate) fn new(receiver: R) -> Self {
        Self {
            receiver,
            marker: PhantomData,
        }
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
    R: AsyncUnboundedReceiver<T::Item>,
{
    type Item = T::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(cx)
    }
}
