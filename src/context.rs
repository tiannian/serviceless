use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

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
        let (sender, receiver) = unbounded_channel();

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

    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Stop an service
    pub fn stop(&mut self) {
        self.receiver.close()
    }
}

impl<S> Context<S>
where
    S: Service + Send,
{
    /// Start an service
    pub fn run(self, service: S) -> Address<S> {
        let mut this = self;

        let address = this.addr();

        let mut service = service;

        tokio::spawn(async move {
            service.started(&mut this).await;
            while let Some(mut e) = this.receiver.recv().await {
                e.handle(&mut service, &mut this).await;
            }
            service.stopped(&mut this).await;
        });

        address
    }
}
