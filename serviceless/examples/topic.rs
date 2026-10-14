use async_trait::async_trait;
use serviceless::{
    service, Context, Handler, Message, Service, ServiceAddress, Topic, TopicEndpoint, TopicRoutes,
};

#[derive(Clone)]
pub struct UserReady(pub String);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserReady)]
pub struct UserReadyTopic(pub String);

#[derive(Default, TopicRoutes)]
pub struct MyService {
    #[topic]
    pub user_ready: TopicEndpoint<UserReadyTopic>,
}

#[service(name = "my_service")]
impl Service for MyService {}

#[derive(Message)]
#[message(result = ())]
pub struct DoWork;

#[async_trait]
impl Handler<DoWork> for MyService {
    async fn handle(&mut self, _message: DoWork, ctx: &mut Context<Self>) {
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
    let (addr, future) = Context::new(&service).run(service);
    tokio::spawn(future);
    demo(addr).await;
}
