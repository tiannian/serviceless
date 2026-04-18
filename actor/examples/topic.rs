use async_trait::async_trait;
use serviceless::{Context, Handler, RoutedTopic, Service, ServiceAddress, Topic, TopicEndpoint};

#[derive(Clone)]
pub struct UserReady(pub String);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UserReadyTopic(pub String);

impl Topic for UserReadyTopic {
    type Item = UserReady;
}

#[derive(Default)]
pub struct MyService {
    pub user_ready: TopicEndpoint<UserReadyTopic>,
}

#[async_trait]
impl Service for MyService {
    type Stream = serviceless::EmptyStream<Self>;
}

impl RoutedTopic<MyService> for UserReadyTopic {
    fn endpoint(service: &mut MyService) -> &mut TopicEndpoint<Self> {
        &mut service.user_ready
    }
}

pub struct DoWork;

impl serviceless::Message for DoWork {
    type Result = ();
}

#[async_trait]
impl Handler<DoWork> for MyService {
    async fn handle(&mut self, _message: DoWork, ctx: &mut Context<Self, Self::Stream>) {
        let _ = ctx.publish_handle().publish(
            UserReadyTopic("user-42".to_string()),
            UserReady("done".to_string()),
        );
    }
}

async fn demo(addr: ServiceAddress<MyService>) {
    let fut = addr
        .subscribe::<UserReadyTopic>(UserReadyTopic("user-42".to_string()))
        .expect("subscribe should enqueue");
    let _ = addr.send(DoWork);
    let event = fut.await.expect("topic item");
    assert_eq!(event.0, "done");
}

#[tokio::main]
async fn main() {
    let service = MyService::default();
    let (addr, future) = service.start_by_context(Context::new());
    tokio::spawn(future);
    demo(addr).await;
}
