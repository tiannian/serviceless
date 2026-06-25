use async_trait::async_trait;

use crate::runtime::InnerOp;

pub trait OneshotSender<T>: InnerOp + Send {
    type Error;

    fn send(self, item: T) -> Result<(), Self::Error>;

    fn is_closed(&self) -> bool;
}

#[async_trait]
pub trait OneshotReceiver<T>: InnerOp + Send {
    type Error;

    async fn recv(self) -> Result<T, Self::Error>;

    fn close(&mut self);
}
