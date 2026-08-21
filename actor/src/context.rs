use futures_util::{
    stream::{empty, select, Empty, Select},
    StreamExt,
};
use prometheus_client::registry::Registry;
use std::{future::Future, time::Instant};
use tracing::{debug, trace};

use crate::{
    metrics::Metrics,
    runtime::{Runtime, Spawner, UnboundedReceiver, UnboundedSender},
    Envelope, Error, RoutedTopic, RuntimedService, ServiceAddress, Topic,
};

/// Context to run service
pub struct Context<S>
where
    S: RuntimedService,
{
    sender: <S::Runtime as Runtime>::UnboundedSender<Envelope<S>>,
    receiver: Select<<S::Runtime as Runtime>::UnboundedReceiver<Envelope<S>>, S::Stream>,
    tasks: <S::Runtime as Runtime>::Spawner<()>,

    metrics: Metrics,

    stopped: bool,
}

impl<S> Context<S>
where
    S: RuntimedService<Stream = Empty<Envelope<S>>>,
{
    /// Create an empty context
    pub fn new(service: &S) -> Self {
        Self::with_stream(service, empty(), None)
    }

    pub fn new_with_registry(service: &S, registry: &mut Registry) -> Self {
        Self::with_stream(service, empty(), Some(registry))
    }

    pub fn new_with_registry_opt(service: &S, registry: Option<&mut Registry>) -> Self {
        Self::with_stream(service, empty(), registry)
    }
}

impl<S> Context<S>
where
    S: RuntimedService,
{
    /// Create a context with an additional stream of envelopes.
    pub fn with_stream(service: &S, stream: S::Stream, registry: Option<&mut Registry>) -> Self {
        let (sender, receiver) = <S::Runtime as Runtime>::unbounded();

        let ctx: Context<S> = Self {
            sender,
            receiver: select(receiver, stream),
            tasks: <S::Runtime as Runtime>::spawner(),

            metrics: Metrics::new(),
            stopped: false,
        };

        let metadata = service.metadata();

        if let Some(registry) = registry {
            ctx.metrics.register(metadata.name, registry);
        }

        ctx
    }

    /// Get service's address
    ///
    /// Even if service not start, you can also get an address.
    /// But if you send message, the message maybe lost.
    pub fn addr(&self) -> ServiceAddress<S> {
        ServiceAddress {
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

        self.stopped = true;
    }

    /// Mutable reference to the extra envelope stream from [`Self::with_stream`].
    ///
    /// Incoming mail from [`ServiceAddress`] is merged with this stream internally;
    /// it is not exposed here—only the user half `T` is.
    pub fn stream(&mut self) -> &mut S::Stream {
        let (_, stream) = self.receiver.get_mut();
        stream
    }

    pub fn spawner(&mut self) -> &mut <S::Runtime as Runtime>::Spawner<()> {
        &mut self.tasks
    }

    pub(crate) fn receiver(
        &mut self,
    ) -> &mut <S::Runtime as Runtime>::UnboundedReceiver<Envelope<S>> {
        let (receiver, _) = self.receiver.get_mut();
        receiver
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
    pub fn run(self, service: S) -> (ServiceAddress<S>, impl Future<Output = ()> + Send) {
        let mut this = self;

        let address = this.addr();

        let mut service = service;

        let service_name = String::from(service.metadata().name);

        let future = async move {
            service.started(&mut this).await;

            loop {
                while let Some(_res) = this.tasks.try_join_next() {}

                trace!(target: "serviceless", "looping once begin");
                tokio::select! {
                    biased;

                    Some(e) = this.receiver.next() => {
                        let pending_tasks = this.tasks.len();
                        this.metrics.pending_tasks.set(pending_tasks as i64);

                        let pending_messages = this.receiver().len();
                        this.metrics.pending_messages.set(pending_messages as i64);

                        let start_time = Instant::now();

                        debug!(target: "serviceless", "Received envelope from {}", service_name);

                        e.handle(&mut service, &mut this).await;

                        debug!(target: "serviceless", "Handled envelope from {}", service_name);

                        let duration = start_time.elapsed();
                        this.metrics
                            .message_processing_time
                            .observe(duration.as_secs_f64());

                        this.metrics.processed_messages.inc();

                        let pending_tasks = this.tasks.len();
                        this.metrics.pending_tasks.set(pending_tasks as i64);

                        if this.stopped {
                            break;
                        }
                    }

                    Some(_res) = this.tasks.join_next(), if !this.tasks.is_empty() => {}

                }
                trace!(target: "serviceless", "looping once end");
            }

            let pending_tasks = this.tasks.len();
            this.metrics.pending_tasks.set(pending_tasks as i64);

            service.stopped(&mut this).await;

            let pending_tasks = this.tasks.len();
            this.metrics.pending_tasks.set(pending_tasks as i64);

            while this.tasks.join_next().await.is_some() {}
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
