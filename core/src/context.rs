use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{Address, EnvelopProxy, Envelope, Service};

pub trait ServiceContext<S>: Sized {
    type Config: Default;

    /// Create an empty context
    fn new(config: Self::Config) -> Self;

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    fn addr(&self) -> Address<S>;

    /// Stop an service
    fn stop(&mut self);

    /// Start an service
    fn run(self, service: S) -> Address<S>;
}

/// Context to run service
pub struct Context<S> {
    sender: UnboundedSender<Envelope<S>>,
    receiver: UnboundedReceiver<Envelope<S>>,
}

impl<S> ServiceContext<S> for Context<S>
where
    S: Service + Send,
{
    type Config = ();

    fn new(_config: ()) -> Self {
        let (sender, receiver) = unbounded_channel();

        Self { sender, receiver }
    }

    fn addr(&self) -> Address<S> {
        Address {
            sender: self.sender.clone(),
        }
    }

    fn stop(&mut self) {
        self.receiver.close()
    }

    fn run(self, service: S) -> Address<S> {
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
