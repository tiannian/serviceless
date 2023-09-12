use async_trait::async_trait;

use crate::Service;

/// Handler message on service
#[async_trait]
pub trait Handler<M>
where
    Self: Service + Sized,
    M: Message,
{
    /// Handle message
    async fn handler(&mut self, message: M, ctx: &mut Self::Runtime) -> M::Result;
}

/// Message
pub trait Message {
    /// Result of message
    type Result;
}
