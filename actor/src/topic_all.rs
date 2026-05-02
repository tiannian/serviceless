use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use futures_util::StreamExt;
use service_channel::mpsc;

use crate::Topic;

pub struct TopicAllHandle<T: Topic> {
    receiver: mpsc::UnboundedReceiver<T::Item>,
}

impl<T> TopicAllHandle<T>
where
    T: Topic,
{
    pub(crate) fn new(receiver: mpsc::UnboundedReceiver<T::Item>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Option<T::Item> {
        self.receiver.recv().await.ok()
    }

    pub fn close(&mut self) {
        self.receiver.close();
    }
}

impl<T> Stream for TopicAllHandle<T>
where
    T: Topic,
{
    type Item = T::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(cx)
    }
}
