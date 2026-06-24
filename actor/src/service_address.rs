use futures_util::TryFutureExt;
use std::future::Future;

use crate::{envelop::Envelope, Error, Message, Result, Runtime, RuntimedHandler, RuntimedService};
use crate::{OneshotReceiver, RoutedTopic, Topic, TopicAllHandle, UnboundedSender};

/// Address of Service
///
/// This address can clone.
pub struct RuntimedServiceAddress<S, R>
where
    R: Runtime,
{
    pub(crate) sender: R::UnboundedSender<Envelope<S, R>>,
}

impl<S, R> Clone for RuntimedServiceAddress<S, R>
where
    R: Runtime,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<S, R> RuntimedServiceAddress<S, R>
where
    R: Runtime,
{
    /// Return true when service stopped.
    pub fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }

    /// Close the service channel
    pub fn close_service(&self) {
        let _ = self.sender.send(Envelope::StopService);
    }
}

impl<S, R> RuntimedServiceAddress<S, R>
where
    S: RuntimedService<R>,
    R: Runtime,
{
    /// Call service's handler and get result
    pub async fn call<M>(&self, message: M) -> Result<M::Result>
    where
        M: Message + Send + 'static,
        S: RuntimedHandler<M, R>,
        M::Result: Send,
    {
        let (sender, receiver) = R::oneshot::<M::Result>();

        let env = Envelope::new_with_result_channel(message, Some(sender));

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        receiver.recv().await.map_err(|_| Error::ServiceStoped)
    }

    /// Call service's handler without result
    ///
    /// Because this function don't need result, so it can call without async.
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        S: RuntimedHandler<M, R>,
        M::Result: Send,
    {
        let env = Envelope::new(message);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }

    /// Subscribe once to a specific topic value.
    ///
    /// One call waits for one future publication.
    pub fn subscribe<T>(&self, topic: T) -> Result<impl Future<Output = Result<T::Item>> + Send>
    where
        T: Topic + RoutedTopic<S, R>,
    {
        let (sender, receiver) = R::oneshot::<T::Item>();
        let env = Envelope::<S, R>::new_subscribe_topic::<T>(topic, sender);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(receiver.recv().map_err(|_| Error::ServiceStoped))
    }

    pub fn subscribe_all<T>(&self, topic: T) -> Result<TopicAllHandle<T, R>>
    where
        T: Topic + RoutedTopic<S, R>,
    {
        let (sender, receiver) = R::unbounded::<T::Item>();
        let env = Envelope::<S, R>::new_subscribe_all_topic::<T>(topic, sender);
        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;
        Ok(TopicAllHandle::new(receiver))
    }
}
