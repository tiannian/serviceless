use futures_util::{
    stream::{empty, select, Empty, Select},
    Stream, StreamExt,
};
use service_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::future::Future;

use crate::{Envelope, Service, ServiceAddress};

pub trait Context<S>: Send {
    fn addr(&self) -> ServiceAddress<S>;

    fn stop(&mut self);
}

/// Context to run service
pub struct ContextRuntime<S, T = Empty<Envelope<S>>>
where
    T: Stream<Item = Envelope<S>> + Unpin,
{
    sender: UnboundedSender<Envelope<S>>,
    receiver: Select<UnboundedReceiver<Envelope<S>>, T>,
}

impl<S> Default for ContextRuntime<S, Empty<Envelope<S>>> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ContextRuntime<S, Empty<Envelope<S>>> {
    /// Create an empty context
    pub fn new() -> Self {
        Self::with_stream(empty())
    }
}

impl<S, T> ContextRuntime<S, T>
where
    T: Stream<Item = Envelope<S>> + Unpin,
{
    /// Create a context with an additional stream of envelopes.
    pub fn with_stream(stream: T) -> Self {
        let (sender, receiver) = unbounded();

        Self {
            sender,
            receiver: select(receiver, stream),
        }
    }

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    pub(crate) fn _addr(&self) -> ServiceAddress<S> {
        ServiceAddress {
            sender: self.sender.clone(),
        }
    }

    /// Stop an service
    fn _stop(&mut self) {
        self.sender.close_channel()
    }
}

impl<S, T> Context<S> for ContextRuntime<S, T>
where
    T: Stream<Item = Envelope<S>> + Unpin + Send,
{
    fn addr(&self) -> ServiceAddress<S> {
        self._addr()
    }

    fn stop(&mut self) {
        self._stop()
    }
}

impl<S, T> ContextRuntime<S, T>
where
    S: Service + Send,
    T: Stream<Item = Envelope<S>> + Unpin + Send,
{
    /// Start an service
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    pub fn run(self, service: S) -> (ServiceAddress<S>, impl Future<Output = ()> + Send) {
        let mut this = self;

        let address = this._addr();

        let mut service = service;

        let future = async move {
            service.started(&mut this).await;
            while let Some(e) = this.receiver.next().await {
                e.handle(&mut service, &mut this).await;
            }
            service.stopped(&mut this).await;
        };

        (address, future)
    }
}
