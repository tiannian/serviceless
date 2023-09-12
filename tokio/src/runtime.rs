use serviceless_core::{Runtime, Service};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{envelope::EnvelopProxy, Envelope, TokioAddress};

/// Context to run service
pub struct TokioRuntime<S> {
    sender: UnboundedSender<Envelope<S>>,
    receiver: UnboundedReceiver<Envelope<S>>,
}

impl<S> Default for TokioRuntime<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> TokioRuntime<S> {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded_channel();

        Self { sender, receiver }
    }
}

impl<S> Runtime<S> for TokioRuntime<S>
where
    S: Service<Runtime = Self> + Send,
{
    type Address = TokioAddress<S>;

    fn addr(&self) -> TokioAddress<S> {
        TokioAddress {
            sender: self.sender.clone(),
        }
    }

    fn stop(&mut self) {
        self.receiver.close()
    }

    fn run(self, service: S) -> TokioAddress<S> {
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
