use futures_util::{
    stream::{empty, select, Empty, Select},
    StreamExt,
};
use std::future::Future;

use crate::{
    runtime::{Runtime, Spawner, UnboundedReceiver, UnboundedSender},
    Envelope, Error, RoutedTopic, RuntimedService, RuntimedServiceAddress, Topic,
};

/// Context to run service
pub struct Context<S>
where
    S: RuntimedService,
{
    sender: <S::Runtime as Runtime>::UnboundedSender<Envelope<S>>,
    receiver: Select<<S::Runtime as Runtime>::UnboundedReceiver<Envelope<S>>, S::Stream>,
    tasks: <S::Runtime as Runtime>::Spawner<Result<(), S::Error>>,
}

impl<S> Default for Context<S>
where
    S: RuntimedService<Stream = Empty<Envelope<S>>>,
{
    /// Equivalent to [`Context::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Context<S>
where
    S: RuntimedService<Stream = Empty<Envelope<S>>>,
{
    /// Create an empty context
    pub fn new() -> Self {
        Self::with_stream(empty())
    }
}

impl<S> Context<S>
where
    S: RuntimedService,
{
    /// Create a context with an additional stream of envelopes.
    pub fn with_stream(stream: S::Stream) -> Self {
        let (sender, receiver) = <S::Runtime as Runtime>::unbounded();

        Self {
            sender,
            receiver: select(receiver, stream),
            tasks: <S::Runtime as Runtime>::spawner(),
        }
    }

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    pub fn addr(&self) -> RuntimedServiceAddress<S> {
        RuntimedServiceAddress {
            sender: self.sender.clone(),
        }
    }

    /// Get a publish handle
    pub fn publish_handle(&self) -> PublishHandle<S>
    where
        S: RuntimedService,
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
    pub fn stream(&mut self) -> &mut S::Stream {
        let (_, stream) = self.receiver.get_mut();
        stream
    }

    pub fn spawner(&mut self) -> &mut impl Spawner<Result<(), S::Error>> {
        &mut self.tasks
    }
}

impl<S> Context<S>
where
    S: RuntimedService,
{
    /// Start an service
    ///
    /// Returns the address and a future that should be spawned to run the service.
    /// The caller is responsible for spawning the returned future using their async runtime.
    pub fn run(
        self,
        service: S,
    ) -> (
        RuntimedServiceAddress<S>,
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

            while this.tasks.join_next().await.is_some() {}

            Ok(())
        };

        (address, future)
    }
}

pub struct PublishHandle<S>
where
    S: RuntimedService,
{
    pub(crate) sender: <S::Runtime as Runtime>::UnboundedSender<Envelope<S>>,
}

impl<S> PublishHandle<S>
where
    S: RuntimedService,
{
    /// Publish one item to a specific topic value.
    ///
    /// The actual delivery is still serialized through the service mailbox.
    pub fn publish<TopicT>(&self, topic: TopicT, item: TopicT::Item) -> Result<(), Error>
    where
        TopicT: Topic + RoutedTopic<S>,
        S: RuntimedService,
    {
        let env = Envelope::<S>::new_publish_topic::<TopicT>(topic, item);

        self.sender.send(env).map_err(|_| Error::ServiceStoped)?;

        Ok(())
    }
}
