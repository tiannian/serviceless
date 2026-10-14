use serviceless::{
    runtime::Runtime, service, Context, EmptyStream, Envelope, Message, Metadata, RoutedTopic,
    RuntimedTopicEndpoint, Service, TokioRuntime, Topic, TopicRoutes,
};
use std::marker::PhantomData;

struct DefaultService;

#[service]
impl Service for DefaultService {}

struct GenericService<T>(PhantomData<T>);

#[service]
impl<T: Send + Sync + 'static> Service for GenericService<T> {}

struct SelectedRuntime;

impl Runtime for SelectedRuntime {
    type Error = <TokioRuntime as Runtime>::Error;

    type UnboundedSender<T>
        = <TokioRuntime as Runtime>::UnboundedSender<T>
    where
        T: Send;
    type UnboundedReceiver<T>
        = <TokioRuntime as Runtime>::UnboundedReceiver<T>
    where
        T: Send;

    fn unbounded<T>() -> (Self::UnboundedSender<T>, Self::UnboundedReceiver<T>)
    where
        T: Send,
    {
        TokioRuntime::unbounded()
    }

    type OneshotSender<T>
        = <TokioRuntime as Runtime>::OneshotSender<T>
    where
        T: Send;
    type OneshotReceiver<T>
        = <TokioRuntime as Runtime>::OneshotReceiver<T>
    where
        T: Send;

    fn oneshot<T>() -> (Self::OneshotSender<T>, Self::OneshotReceiver<T>)
    where
        T: Send,
    {
        TokioRuntime::oneshot()
    }

    type Spawner<T>
        = <TokioRuntime as Runtime>::Spawner<T>
    where
        T: Send + 'static;

    fn spawner<T>() -> Self::Spawner<T>
    where
        T: Send + 'static,
    {
        TokioRuntime::spawner()
    }
}

struct ConfiguredService;

#[service(runtime = "SelectedRuntime", name = "configured")]
impl Service for ConfiguredService {
    type Stream = EmptyStream<Self>;
}

struct CustomMetadata;

#[service]
impl Service for CustomMetadata {
    type Stream = futures_util::stream::Empty<Envelope<Self>>;

    fn metadata(&self) -> Metadata<'_> {
        Metadata { name: "custom" }
    }

    async fn started(&mut self, _ctx: &mut Context<Self>) {}
}

#[derive(Message)]
#[message(result = T)]
struct GenericMessage<T>(T);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = u32)]
struct NumberTopic(u32);

#[derive(Default, TopicRoutes)]
struct RoutedService {
    #[topic]
    numbers: RuntimedTopicEndpoint<NumberTopic, SelectedRuntime>,
}

#[service(runtime = "SelectedRuntime")]
impl Service for RoutedService {}

fn assert_tokio_service<T: Service<Runtime = TokioRuntime>>() {}

#[test]
fn defaults_supply_runtime_stream_and_type_name() {
    assert_tokio_service::<DefaultService>();
    let service = DefaultService;
    assert_eq!(service.metadata().name, "DefaultService");
    let _context = Context::new(&service);

    let generic = GenericService::<u32>(PhantomData);
    assert_eq!(generic.metadata().name, "GenericService");
}

#[test]
fn options_and_explicit_members_are_preserved() {
    fn assert_selected_service<T: Service<Runtime = SelectedRuntime>>() {}
    assert_selected_service::<ConfiguredService>();
    assert_tokio_service::<CustomMetadata>();
    assert_eq!(ConfiguredService.metadata().name, "configured");
    assert_eq!(CustomMetadata.metadata().name, "custom");
}

#[test]
fn derived_message_topic_and_route_keep_explicit_storage() {
    fn assert_result<M: Message<Result = u32>>() {}
    fn assert_item<T: Topic<Item = u32>>() {}
    assert_result::<GenericMessage<u32>>();
    assert_item::<NumberTopic>();

    let mut service = RoutedService::default();
    let _: &mut RuntimedTopicEndpoint<NumberTopic, SelectedRuntime> =
        NumberTopic::endpoint(&mut service);
}
