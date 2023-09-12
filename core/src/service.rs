use async_trait::async_trait;

use crate::{Address, Context, ServiceContext};

/// A service is an running like thread
#[async_trait]
pub trait Service: Send + Sized + 'static {
    type Context: ServiceContext<Self>;

    /// Start service
    fn start(self) -> Address<Self> {
        Context::new(()).run(self)
    }

    fn start_by_context(self, ctx: Self::Context) -> Address<Self> {
        ctx.run(self)
    }

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Context<Self>) {}

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {}
}
