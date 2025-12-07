use async_trait::async_trait;
use std::future::Future;

use crate::{Address, Context};

/// A service is an running like thread
#[async_trait]
pub trait Service: Send + Sized + 'static {
    /// Start a service with a new context
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    fn start(self) -> (Address<Self>, impl Future<Output = ()> + Send) {
        Context::new().run(self)
    }

    /// Start a service with the given context
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    fn start_by_context(
        self,
        ctx: Context<Self>,
    ) -> (Address<Self>, impl Future<Output = ()> + Send) {
        ctx.run(self)
    }

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Context<Self>) {}

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {}
}
