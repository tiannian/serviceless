use async_trait::async_trait;
use alloc::boxed::Box;

use crate::{Context, Service};

/// Handler message on service
#[async_trait]
pub trait Handler<M>
where
    Self: Service + Sized,
    M: Message,
{
    /// Handle message
    async fn handle(&mut self, message: M, ctx: &mut Context<Self>) -> M::Result;
}

/// Message
pub trait Message {
    /// Result of message
    type Result;
}
