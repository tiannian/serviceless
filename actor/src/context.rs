use futures_util::StreamExt;
use core::future::Future;
use service_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};

use crate::{Address, EnvelopProxy, Envelope, Service};

/// Context to run service
pub struct Context<S> {
    sender: UnboundedSender<Envelope<S>>,
    receiver: UnboundedReceiver<Envelope<S>>,
    pub(crate) paused: bool,
}

impl<S> Default for Context<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Context<S> {
    /// Create an empty context
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self {
            sender,
            receiver,
            paused: false,
        }
    }

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    pub fn addr(&self) -> Address<S> {
        Address {
            sender: self.sender.clone(),
        }
    }

    /// Pause context
    ///
    /// Notice: This funcion is unusable now.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Stop an service
    pub fn stop(&mut self) {
        self.sender.close_channel()
    }
}

impl<S> Context<S>
where
    S: Service + Send,
{
    /// Start an service
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    pub fn run(self, service: S) -> (Address<S>, impl Future<Output = ()> + Send) {
        let mut this = self;

        let address = this.addr();

        let mut service = service;

        let future = async move {
            service.started(&mut this).await;
            while let Some(mut e) = this.receiver.next().await {
                e.handle(&mut service, &mut this).await;
            }
            service.stopped(&mut this).await;
        };

        (address, future)
    }
}
