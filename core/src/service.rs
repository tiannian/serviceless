use async_trait::async_trait;

use crate::ServiceRuntime;

/// A service is an running like thread
#[async_trait]
pub trait Service: Send + Sized + 'static {
    type Runtime: ServiceRuntime<Self>;

    fn start(self, ctx: Self::Runtime) -> AddressOfService<Self> {
        ctx.run(self)
    }

    /// Hook for service started
    async fn started(&mut self, _ctx: &mut Self::Runtime) {}

    /// Hook for service stopped
    async fn stopped(&mut self, _ctx: &mut Self::Runtime) {}
}

/// Address type of service
pub type AddressOfService<S> = <<S as Service>::Runtime as ServiceRuntime<S>>::Address;
