use futures_util::StreamExt;
use service_channel::mpsc::{unbounded, UnboundedSender};
use service_channel::oneshot;
use std::future::Future;

use crate::{
    address::Address, envelop::{Envelope, EnvelopWithMessage}, Error, Handler, Message, Result, Service,
};

/// Address of Service
///
/// This address can clone.
pub struct ServiceAddress<S> {
    pub(crate) sender: UnboundedSender<Envelope<S>>,
}

impl<S> Clone for ServiceAddress<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<S> ServiceAddress<S> {
    /// Return true when service stopped.
    pub fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }
}

impl<S> ServiceAddress<S>
where
    S: Service,
{
    /// Call service's handler and get result
    pub async fn call<M>(&self, message: M) -> Result<M::Result>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
        let (sender, receiver) = oneshot::channel::<M::Result>();

        let env = Envelope::new(message, Some(sender));

        self.sender
            .unbounded_send(env)
            .map_err(|_| Error::ServiceStoped)?;

        receiver.await.map_err(|_| Error::ServiceStoped)
    }

    /// Call service's handler without result
    ///
    /// Because this function don't need result, so it can call without async.
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
        let env = Envelope::new(message, None);

        self.sender
            .unbounded_send(env)
            .map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }

    /// Convert ServiceAddress to Address for a specific message type
    ///
    /// This creates a forwarding task that receives messages from the Address
    /// and forwards them to the ServiceAddress. The returned Address can only
    /// send messages of type M.
    ///
    /// Returns the Address and a Future that should be spawned to run the forwarding task.
    pub fn into_address<M>(self) -> (Address<M>, impl Future<Output = ()> + Send)
    where
        M: Message + Send + 'static,
        S: Handler<M> + Send,
        M::Result: Send,
    {
        let (sender, mut receiver) = unbounded::<Box<EnvelopWithMessage<M>>>();
        let service_sender = self.sender;

        let address = Address { sender };

        let future = async move {
            while let Some(boxed_env) = receiver.next().await {
                // Convert Box<EnvelopWithMessage<M>> to Envelope<S> without re-boxing
                let envelope = Envelope::from_boxed(boxed_env);
                if service_sender.unbounded_send(envelope).is_err() {
                    // Service stopped, break the forwarding loop
                    break;
                }
            }
        };

        (address, future)
    }
}
