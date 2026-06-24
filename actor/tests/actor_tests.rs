use async_trait::async_trait;
use serviceless::{
    Context, EmptyStream, Handler, Message, Metadata, ReplyHandle, RoutedTopic, Service, Topic,
    TopicEndpoint,
};

#[derive(Default)]
struct TestActor {
    value: i32,
    numbers: TopicEndpoint<NumberTopic>,
    preferred_used: bool,
}

#[async_trait]
impl Service for TestActor {
    type Stream = EmptyStream<Self>;

    type Error = ();

    fn metadata(&self) -> Metadata<'_> {
        Metadata { name: "test_actor" }
    }
}

struct Add(i32);
impl Message for Add {
    type Result = i32;
}

#[async_trait]
impl Handler<Add> for TestActor {
    async fn handle(&mut self, msg: Add, _ctx: &mut Context<Self>) -> i32 {
        self.value += msg.0;
        self.value
    }
}

struct Get;
impl Message for Get {
    type Result = i32;
}

#[async_trait]
impl Handler<Get> for TestActor {
    async fn handle(&mut self, _msg: Get, _ctx: &mut Context<Self>) -> i32 {
        self.value
    }
}

struct PreferredUsed;
impl Message for PreferredUsed {
    type Result = bool;
}

#[async_trait]
impl Handler<PreferredUsed> for TestActor {
    async fn handle(&mut self, _msg: PreferredUsed, _ctx: &mut Context<Self>) -> bool {
        self.preferred_used
    }
}

struct PreferredAdd(i32);
impl Message for PreferredAdd {
    type Result = i32;
}

#[async_trait]
impl Handler<PreferredAdd> for TestActor {
    async fn handle(&mut self, msg: PreferredAdd, _ctx: &mut Context<Self>) -> i32 {
        self.value += msg.0;
        self.value
    }

    async fn handle_preferred(
        &mut self,
        msg: PreferredAdd,
        _ctx: &mut Context<Self>,
        handle: ReplyHandle<PreferredAdd>,
    ) {
        self.preferred_used = true;
        self.value += msg.0;
        let _ = handle.send(self.value);
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum NumberTopic {
    Even,
    Odd,
}

impl Topic for NumberTopic {
    type Item = i32;
}

impl RoutedTopic<TestActor> for NumberTopic {
    fn endpoint(service: &mut TestActor) -> &mut TopicEndpoint<Self> {
        &mut service.numbers
    }
}

struct PublishNumber {
    topic: NumberTopic,
    value: i32,
}

impl Message for PublishNumber {
    type Result = ();
}

#[async_trait]
impl Handler<PublishNumber> for TestActor {
    async fn handle(&mut self, msg: PublishNumber, ctx: &mut Context<Self>) {
        let _ = ctx.publish_handle().publish(msg.topic, msg.value);
    }
}

#[tokio::test]
async fn call_and_send_update_actor_state() {
    let (addr, run) = TestActor::default().start_by_context(Context::new());
    let runner = tokio::spawn(run);

    let v = addr.call(Add(2)).await.expect("call add should succeed");
    assert_eq!(v, 2);

    addr.send(Add(3)).expect("send add should succeed");
    let final_value = addr.call(Get).await.expect("get should succeed");
    assert_eq!(final_value, 5);

    addr.close_service();
    runner.await.expect("service task should finish").unwrap();
}

#[tokio::test]
async fn preferred_handler_path_is_used_for_preferred_messages() {
    let (addr, run) = TestActor::default().start_by_context(Context::new());
    let runner = tokio::spawn(run);

    let v = addr
        .call(PreferredAdd(7))
        .await
        .expect("preferred call should succeed");
    assert_eq!(v, 7);

    let state = addr.call(Get).await.expect("get should succeed");
    assert_eq!(state, 7);

    let used = addr
        .call(PreferredUsed)
        .await
        .expect("preferred status should be readable");
    assert!(used);

    addr.close_service();
    runner.await.expect("service task should finish").unwrap();
}

#[tokio::test]
async fn subscribe_receives_published_topic_item() {
    let (addr, run) = TestActor::default().start_by_context(Context::new());
    let runner = tokio::spawn(run);

    let sub_addr = addr.clone();
    let subscriber = tokio::spawn(async move {
        sub_addr
            .subscribe(NumberTopic::Even)
            .expect("subscribe should enqueue")
            .await
            .expect("subscribe should receive published value")
    });

    tokio::task::yield_now().await;
    addr.send(PublishNumber {
        topic: NumberTopic::Even,
        value: 42,
    })
    .expect("publish message should enqueue");

    let published = subscriber.await.expect("subscriber task should finish");
    assert_eq!(published, 42);

    addr.close_service();
    runner.await.expect("service task should finish").unwrap();
}

#[tokio::test]
async fn subscribe_all_receives_every_publish_for_topic() {
    let (addr, run) = TestActor::default().start_by_context(Context::new());
    let runner = tokio::spawn(run);

    let mut handle = addr
        .subscribe_all(NumberTopic::Even)
        .expect("subscribe_all should enqueue");

    tokio::task::yield_now().await;

    addr.send(PublishNumber {
        topic: NumberTopic::Even,
        value: 1,
    })
    .expect("first publish should enqueue");
    addr.send(PublishNumber {
        topic: NumberTopic::Even,
        value: 2,
    })
    .expect("second publish should enqueue");

    assert_eq!(handle.recv().await, Some(1));
    assert_eq!(handle.recv().await, Some(2));

    addr.close_service();
    runner.await.expect("service task should finish").unwrap();
}

#[tokio::test]
async fn subscribe_all_only_receives_matching_topic_key() {
    let (addr, run) = TestActor::default().start_by_context(Context::new());
    let runner = tokio::spawn(run);

    let mut handle = addr
        .subscribe_all(NumberTopic::Even)
        .expect("subscribe_all should enqueue");

    tokio::task::yield_now().await;

    addr.send(PublishNumber {
        topic: NumberTopic::Odd,
        value: 999,
    })
    .expect("publish to other topic key should enqueue");
    addr.send(PublishNumber {
        topic: NumberTopic::Even,
        value: 42,
    })
    .expect("publish to subscribed key should enqueue");

    assert_eq!(handle.recv().await, Some(42));

    addr.close_service();
    runner.await.expect("service task should finish").unwrap();
}
