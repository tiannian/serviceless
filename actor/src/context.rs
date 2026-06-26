use futures_util::{
    stream::{empty, select, Empty, Select},
    StreamExt,
};
use prometheus_client::{
    metrics::{
        counter::Counter,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};
use std::{future::Future, time::Instant};

use crate::{
    runtime::{Runtime, Spawner, UnboundedReceiver, UnboundedSender},
    Envelope, Error, RoutedTopic, RuntimedService, ServiceAddress, Topic,
};

struct Metrics {
    pub pending_tasks: Gauge,

    pub message_processing_time: Histogram,
    pub pending_messages: Gauge,
    pub processed_messages: Counter,
}

impl Metrics {
    pub fn new() -> Self {
        let pending_tasks = Gauge::default();
        let message_processing_time = Histogram::new(exponential_buckets(
            0.0001, // 100us
            2.0, 16,
        ));
        let pending_messages = Gauge::default();
        let processed_messages = Counter::default();

        Self {
            pending_tasks,
            message_processing_time,
            pending_messages,
            processed_messages,
        }
    }

    pub fn register(&self, name: &str, registry: &mut Registry) {
        let sub_registry = registry.sub_registry_with_prefix(name);

        sub_registry.register(
            "pending_tasks",
            "Number of pending tasks",
            self.pending_tasks.clone(),
        );

        sub_registry.register(
            "message_processing_time",
            "Time taken to process messages",
            self.message_processing_time.clone(),
        );

        sub_registry.register(
            "pending_messages",
            "Number of pending messages",
            self.pending_messages.clone(),
        );

        sub_registry.register(
            "processed_messages",
            "Number of processed messages",
            self.processed_messages.clone(),
        );
    }
}

/// Context to run service
pub struct Context<S>
where
    S: RuntimedService,
{
    sender: <S::Runtime as Runtime>::UnboundedSender<Envelope<S>>,
    receiver: Select<<S::Runtime as Runtime>::UnboundedReceiver<Envelope<S>>, S::Stream>,
    tasks: <S::Runtime as Runtime>::Spawner<Result<(), S::Error>>,

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
            ctx.metrics.register(&metadata.name, registry);
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

    pub fn spawner(&mut self) -> &mut impl Spawner<Result<(), S::Error>> {
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
    pub fn run(
        self,
        service: S,
    ) -> (
        ServiceAddress<S>,
        impl Future<Output = Result<(), S::Error>> + Send,
    ) {
        let mut this = self;

        let address = this.addr();

        let mut service = service;

        let future = async move {
            service.started(&mut this).await?;

            while let Some(e) = this.receiver.next().await {
                let pending_tasks = this.tasks.len();
                this.metrics.pending_tasks.set(pending_tasks as i64);

                let pending_messages = this.receiver().len();
                this.metrics.pending_messages.set(pending_messages as i64);

                let start_time = Instant::now();
                e.handle(&mut service, &mut this).await;
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

            let pending_tasks = this.tasks.len();
            this.metrics.pending_tasks.set(pending_tasks as i64);

            service.stopped(&mut this).await?;

            let pending_tasks = this.tasks.len();
            this.metrics.pending_tasks.set(pending_tasks as i64);

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
