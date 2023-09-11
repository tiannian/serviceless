use async_trait::async_trait;

use crate::{Context, Service};

/// Handler message on service
#[async_trait]
pub trait Handler<M>
where
    Self: Service + Sized,
    M: Message,
{
    /// Handle message
    async fn handler(&mut self, message: M, ctx: &mut Context<Self>) -> M::Result;
}

pub trait Message {
    type Result;
}
