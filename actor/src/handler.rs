use async_trait::async_trait;

use crate::{Context, RuntimedReplyHandle, RuntimedService};

/// Handler message on service
#[async_trait]
pub trait Handler<M>
where
    Self: RuntimedService + Sized,
    M: Message,
{
    /// Handle message
    async fn handle(&mut self, message: M, ctx: &mut Context<Self>) -> M::Result;

    /// Handle message
    async fn handle_preferred(
        &mut self,
        message: M,
        ctx: &mut Context<Self>,
        handle: RuntimedReplyHandle<M, Self::Runtime>,
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
    /// Result of message
    type Result;
}
