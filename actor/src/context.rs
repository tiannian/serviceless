use futures_util::{
    stream::{empty, select, Empty, Select},
    Stream, StreamExt,
};
use std::{future::Future, marker::PhantomData};

use crate::{
    Envelope, Error, RoutedTopic, Runtime, RuntimedService, RuntimedServiceAddress, Topic,
    UnboundedReceiver, UnboundedSender,
};

/// Context to run service
pub struct Context<S, T, R>
where
    S: RuntimedService<R>,
    T: Stream<Item = Envelope<S, R>> + Unpin,
    R: Runtime,
{
    sender: R::UnboundedSender<Envelope<S, R>>,
    receiver: Select<R::UnboundedReceiver<Envelope<S, R>>, T>,
    marker_runtime: PhantomData<R>,
    tasks: R::Spawner<Result<(), S::Error>>,
}

impl<S, R> Default for Context<S, Empty<Envelope<S, R>>, R>
where
    S: RuntimedService<R>,
    R: Runtime,
{
    /// Equivalent to [`Context::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl<S, R> Context<S, Empty<Envelope<S, R>>, R>
where
    S: RuntimedService<R>,
    R: Runtime,
{
    /// Create an empty context
    pub fn new() -> Self {
        Self::with_stream(empty())
    }
}

impl<S, T, R> Context<S, T, R>
where
    S: RuntimedService<R>,
    T: Stream<Item = Envelope<S, R>> + Unpin,
    R: Runtime,
{
    /// Create a context with an additional stream of envelopes.
    pub fn with_stream(stream: T) -> Self {
        let (sender, receiver) = R::unbounded();

        Self {
            sender,
            receiver: select(receiver, stream),
            marker_runtime: PhantomData,
            tasks: R::spawner(),
        }
    }

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    pub fn addr(&self) -> RuntimedServiceAddress<S, R> {
        RuntimedServiceAddress {
            sender: self.sender.clone(),
        }
    }

    /// Get a publish handle
    pub fn publish_handle(&self) -> PublishHandle<S, R>
    where
        S: RuntimedService<R>,
        R: Runtime,
    {
        PublishHandle {
            sender: self.sender.clone(),
        }
    }

    /// Stop an service
    pub fn stop(&mut self) {
        let (receiver, _) = self.receiver.get_mut();
        receiver.close();
    }

    /// Mutable reference to the extra envelope stream from [`Self::with_stream`].
    ///
    /// Incoming mail from [`ServiceAddress`] is merged with this stream internally;
    /// it is not exposed here—only the user half `T` is.
    pub fn stream(&mut self) -> &mut T {
        let (_, stream) = self.receiver.get_mut();
        stream
    }

    pub fn spawner(&mut self) -> &mut R::Spawner<Result<(), S::Error>> {
        &mut self.tasks
    }
}

impl<S, T, R> Context<S, T, R>
where
    S: RuntimedService<R, Stream = T> + Send + Sized,
    T: Stream<Item = Envelope<S, R>> + Unpin + Send,
    R: Runtime,
{
    /// Start an service
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    pub fn run(
        self,
        service: S,
    ) -> (
        RuntimedServiceAddress<S, R>,
        impl Future<Output = Result<(), S::Error>> + Send,
    ) {
        let mut this = self;

        let address = this.addr();

        let mut service = service;

        let future = async move {
            service.started(&mut this).await?;
            while let Some(e) = this.receiver.next().await {
                e.handle(&mut service, &mut this).await;
            }
            service.stopped(&mut this).await?;

            Ok(())
        };

        (address, future)
    }
}

pub struct PublishHandle<S, R>
where
    S: RuntimedService<R>,
    R: Runtime,
{
    pub(crate) sender: R::UnboundedSender<Envelope<S, R>>,
}

impl<S, R> PublishHandle<S, R>
where
    S: RuntimedService<R>,
    R: Runtime,
{
    /// Publish one item to a specific topic value.
    ///
    /// The actual delivery is still serialized through the service mailbox.
    pub fn publish<TopicT>(&self, topic: TopicT, item: TopicT::Item) -> Result<(), Error>
    where
        TopicT: Topic + RoutedTopic<S, R>,
        S: RuntimedService<R>,
    {
        let env = Envelope::<S, R>::new_publish_topic::<TopicT>(topic, item);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }
}
