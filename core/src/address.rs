use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{envelop::Envelope, Error, Handler, Message, Result, Service};

/// Address of Service
///
/// This address can clone.
pub struct Address<S> {
    pub(crate) sender: UnboundedSender<Envelope<S>>,
}

impl<S> Clone for Address<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<S> Address<S> {
    /// Return true when service stopped.
    pub fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }
}

impl<S> Address<S>
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
        let (sender, receiver) = oneshot::channel();

        let env = Envelope::new(message, Some(sender));

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        receiver.await.map_err(|_| Error::TryToReadSendQueryResult)
    }

    /// Call service's handler without result
    ///
    /// Beacuse this function don't need result, so it can call without async.
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
        let env = Envelope::new(message, None);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }
}
