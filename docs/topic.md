# Topics with macros

[中文](topic.zh.md)

A Topic is a typed publish/subscribe key. Its type specifies the key and event item. A `TopicEndpoint<T>` field on the service stores subscribers, and `#[derive(TopicRoutes)]` connects the topic to that field.

## Complete example

```rust
use async_trait::async_trait;
use serviceless::{
    service, Context, Handler, Message, Service, Topic, TopicEndpoint, TopicRoutes,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct UserReady(String);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserReady)]
struct UserReadyTopic(String);

#[derive(Default, TopicRoutes)]
struct Worker {
    #[topic]
    user_ready: TopicEndpoint<UserReadyTopic>,
}

#[service]
impl Service for Worker {}

#[derive(Message)]
#[message(result = ())]
struct DoWork;

#[async_trait]
impl Handler<DoWork> for Worker {
    async fn handle(&mut self, _message: DoWork, ctx: &mut Context<Self>) {
        ctx.publish_handle()
            .publish(
                UserReadyTopic("user-42".into()),
                UserReady("done".into()),
            )
            .unwrap();
    }
}

#[tokio::main]
async fn main() {
    let worker = Worker::default();
    let (address, run) = Context::new(&worker).run(worker);
    let task = tokio::spawn(run);

    let next = address.subscribe(UserReadyTopic("user-42".into())).unwrap();
    address.send(DoWork).unwrap();
    assert_eq!(next.await.unwrap(), UserReady("done".into()));

    address.close_service();
    task.await.unwrap();
}
```

`#[derive(Topic)]` with `#[topic(item = EventType)]` generates `Topic::Item`. The topic key must implement `Ord + Clone + Send + 'static`; the event item must implement `Clone + Send + 'static`. Here `UserReadyTopic("user-42")` is an exact key used for both subscription and publication. Different keys have separate subscribers.

Derive `TopicRoutes` on a service struct with named fields, and mark each `TopicEndpoint<T>` field with `#[topic]`. The macro implements `RoutedTopic<Worker>` for `T`, pointing to that field. The service still owns and initializes the field, for example through `Default`. With a custom runtime, use `RuntimedTopicEndpoint<T, R>` and select the same runtime in `#[service(runtime = "...")]`.

## One-time and ongoing subscriptions

`address.subscribe(topic)` returns a future for the next publication of that key. Multiple one-time subscribers to the same key all receive that publication; subscribe again to receive a later one. `address.subscribe_all(topic)` returns a `TopicAllHandle<T>`. Call `recv().await` repeatedly, use it as a Stream, or call `close()` to stop receiving.

```rust,ignore
let mut events = address.subscribe_all(UserReadyTopic("user-42".into()))?;
while let Some(event) = events.recv().await {
    println!("{}", event.0);
}
```

Publish with `ctx.publish_handle().publish(topic, item)`. This enqueues the publication in the same service mailbox; success means it was enqueued. Subscriptions also enter that mailbox. Calling `subscribe` or `subscribe_all` before sending the message that triggers publication places those operations in order. A subscriber receives only publications after its registration; topics do not replay earlier events. A publish delivery error generally means the service has stopped.

See the complete [`topic` example](../serviceless/examples/topic.rs). For service and message macros, see the [Service guide](service.md). To expose a topic to JSON-RPC clients, see the [`jsonrpc.topic` guide](jsonrpc-topic.md).
