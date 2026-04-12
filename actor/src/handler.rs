use async_trait::async_trait;

use crate::{Context, ReplyHandle, Service};

/// Handler message on service
#[async_trait]
pub trait Handler<M>
where
    Self: Service + Sized,
    M: Message,
{
    /// Handle message
    async fn handle(&mut self, message: M, ctx: &mut Context<Self, Self::Stream>) -> M::Result;

    /// Handle message
    async fn handle_preferred(
        &mut self,
        message: M,
        ctx: &mut Context<Self, Self::Stream>,
        handle: ReplyHandle<M>,
    ) where
        M: Send + 'static,
        M::Result: Send,
    {
        let res = self.handle(message, ctx).await;
        let _ = handle.send(res);
    }
}

/// Message
pub trait Message {
    const IS_PERFERRED: bool = false;

    /// Result of message
    type Result;
}
