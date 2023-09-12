use async_trait::async_trait;
use serviceless_core::{Address, Handler, Message, Service};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{Envelope, Error, Result};

/// Address of Service
///
/// This address can clone.
pub struct TokioAddress<S> {
    pub(crate) sender: UnboundedSender<Envelope<S>>,
}

impl<S> Clone for TokioAddress<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

#[async_trait]
impl<S> Address<S> for TokioAddress<S>
where
    S: Service,
    S::Runtime: Send,
{
    type Error = Error;

    async fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }

    async fn call<M>(&self, message: M) -> Result<M::Result>
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

    fn send<M>(&self, message: M) -> Result<()>
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
