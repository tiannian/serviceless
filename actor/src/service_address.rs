use futures_util::TryFutureExt;
use std::{future::Future, marker::PhantomData};

use crate::{
    envelop::Envelope,
    runtime::{OneshotReceiver, Runtime, UnboundedSender},
    Error, Handler, Message, Result, RoutedTopic, RuntimedService, RuntimedTopicAllHandle, Topic,
};

/// Address of Service
///
/// This address can clone.
pub struct ServiceAddress<Service, Sender>
where
    Service: RuntimedService,
    Sender: UnboundedSender<Envelope<Service>>,
{
    pub(crate) sender: Sender,
    marker: PhantomData<Service>,
}

impl<Service, Sender> Clone for ServiceAddress<Service, Sender>
where
    Service: RuntimedService,
    Sender: UnboundedSender<Envelope<Service>>,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            marker: PhantomData,
        }
    }
}

impl<Service, Sender> ServiceAddress<Service, Sender>
where
    Service: RuntimedService,
    Sender: UnboundedSender<Envelope<Service>>,
{
    pub fn new(sender: Sender) -> Self {
        Self {
            sender,
            marker: PhantomData,
        }
    }

    /// Return true when service stopped.
    pub fn is_stop(&self) -> bool {
        self.sender.is_closed()
    }

    /// Close the service channel
    pub fn close_service(&self) {
        let _ = self.sender.send(Envelope::StopService);
    }
}

impl<Service, Sender> ServiceAddress<Service, Sender>
where
    Service: RuntimedService,
    Sender: UnboundedSender<Envelope<Service>>,
{
    /// Call service's handler and get result
    pub async fn call<M>(&self, message: M) -> Result<M::Result>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        Service: Handler<M>,
    {
        let (sender, receiver) = <Service::Runtime as Runtime>::oneshot::<M::Result>();

        let env = Envelope::new_with_result_channel(message, Some(sender));

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        receiver.recv().await.map_err(|_| Error::ServiceStoped)
    }

    pub fn call_raw<M>(
        &self,
        message: M,
    ) -> Result<<Service::Runtime as Runtime>::OneshotReceiver<M::Result>>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        Service: Handler<M>,
    {
        let (sender, receiver) = <Service::Runtime as Runtime>::oneshot::<M::Result>();

        let env = Envelope::new_with_result_channel(message, Some(sender));

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(receiver)
    }

    /// Call service's handler without result
    ///
    /// Because this function don't need result, so it can call without async.
    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        Service: Handler<M>,
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
        T: Topic + RoutedTopic<Service>,
    {
        let (sender, receiver) = <Service::Runtime as Runtime>::oneshot::<T::Item>();
        let env = Envelope::<Service>::new_subscribe_topic::<T>(topic, sender);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(receiver.recv().map_err(|_| Error::ServiceStoped))
    }

    pub fn subscribe_all<T>(
        &self,
        topic: T,
    ) -> Result<
        RuntimedTopicAllHandle<T, <Service::Runtime as Runtime>::AsyncUnboundedReceiver<T::Item>>,
    >
    where
        T: Topic + RoutedTopic<Service>,
    {
        let (sender, receiver) = <Service::Runtime as Runtime>::async_unbounded::<T::Item>();
        let env = Envelope::<Service>::new_subscribe_all_topic::<T>(topic, sender);
        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;
        Ok(RuntimedTopicAllHandle::new(receiver))
    }
}
