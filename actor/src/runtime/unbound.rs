use async_trait::async_trait;
use futures_core::Stream;

use crate::runtime::InnerOp;

pub trait UnboundedSender<T>: InnerOp + Clone + Send + Sync {
    type Error;

    fn send(&self, item: T) -> Result<(), Self::Error>;

    fn is_closed(&self) -> bool;
}

#[async_trait]
pub trait UnboundedReceiver<T>: InnerOp + Stream<Item = T> + Unpin + Send + Sync {
    type Error;

    async fn recv(&mut self) -> Result<T, Self::Error>;

    fn close(&mut self);

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
