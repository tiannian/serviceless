use service_channel::{mpsc::UnboundedSender, oneshot};

use crate::{envelop::EnvelopWithMessage, Error, Message, Result};

/// Address for specific message type
///
/// This address is typed with a specific message type M.
pub struct Address<M>
where
    M: Message,
{
    pub(crate) sender: UnboundedSender<EnvelopWithMessage<M>>,
}

impl<M> Clone for Address<M>
where
    M: Message,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<M> Address<M>
where
    M: Message + Send + 'static,
    M::Result: Send,
{
    /// Return true when service stopped.
    pub fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }

    /// Call service's handler and get result
    pub async fn call(&self, message: M) -> Result<M::Result> {
        let (sender, receiver) = oneshot::channel::<M::Result>();

        let env = EnvelopWithMessage::new(message, Some(sender));

        self.sender
            .unbounded_send(env)
            .map_err(|_| Error::ServiceStoped)?;

        receiver.await.map_err(|_| Error::ServicePaused)
    }

    /// Call service's handler without result
    ///
    /// Because this function don't need result, so it can call without async.
    /// If service paused, we have no ServicePaused return.
    pub fn send(&self, message: M) -> Result<()> {
        let env = EnvelopWithMessage::new(message, None);

        self.sender
            .unbounded_send(env)
            .map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }
}
