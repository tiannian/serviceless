use crate::{Service, ServiceAddress};

pub trait ServiceRuntime<S>: Sized
where
    S: Service,
{
    type Address: ServiceAddress<S>;

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    fn addr(&self) -> Self::Address;

    /// Stop an service
    fn stop(&mut self);

    /// Start an service
    fn run(self, service: S) -> Self::Address;
}
