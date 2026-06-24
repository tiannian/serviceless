use async_trait::async_trait;

use crate::{runtime::Runtime, Context, ReplyHandle, RuntimedService};

/// Handler message on service
#[async_trait]
pub trait RuntimedHandler<M, R>
where
    Self: RuntimedService<R> + Sized,
    M: Message,
    R: Runtime,
{
    /// Handle message
    async fn handle(&mut self, message: M, ctx: &mut Context<Self, Self::Stream, R>) -> M::Result;

    /// Handle message
    async fn handle_preferred(
        &mut self,
        message: M,
        ctx: &mut Context<Self, Self::Stream, R>,
        handle: ReplyHandle<M, R>,
    ) where
        M: Send + 'static,
        M::Result: Send,
    {
        let res = self.handle(message, ctx).await;
        let _ = handle.send(res);
    }
}

/// Message
pub trait Message: Send + 'static {
    /// Result of message
    type Result: Send;
}
