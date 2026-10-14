# Topics with macros

[中文](topic.zh.md)

A Topic is a typed publish/subscribe key. The topic type defines the key and event item; a service field stores its subscribers.

## Define the topic and its storage

Derive `Topic` and declare the item type with `#[topic(item = ...)]`:

```rust
use serviceless::Topic;

#[derive(Clone, Debug, PartialEq, Eq)]
struct UserReady(String);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserReady)]
struct UserReadyTopic(String);
```

A topic key must implement `Ord + Clone + Send + 'static`; its item must implement `Clone + Send + 'static`. Here the `String` inside `UserReadyTopic` identifies an exact key, so subscriptions for different users remain separate.

Add a `TopicEndpoint<T>` field to the service and mark it with `#[topic]`:

```rust
use serviceless::{service, Service, TopicEndpoint, TopicRoutes};

#[derive(Default, TopicRoutes)]
struct Worker {
    #[topic]
    user_ready: TopicEndpoint<UserReadyTopic>,
}

#[service]
impl Service for Worker {}
```

`TopicRoutes` implements `RoutedTopic<Worker>` for `UserReadyTopic`, pointing to the marked field. The service owns and initializes that field. For a custom runtime, use `RuntimedTopicEndpoint<T, R>` and select the same runtime with `#[service(runtime = "...")]`.

## Publish from a handler

A handler can get a publish handle from its `Context`. This example publishes an event under the `user-42` key:

```rust
use async_trait::async_trait;
use serviceless::{Context, Handler, Message};

#[derive(Message)]
#[message(result = ())]
struct DoWork;

#[async_trait]
impl Handler<DoWork> for Worker {
    async fn handle(&mut self, _message: DoWork, ctx: &mut Context<Self>) {
        ctx.publish_handle()
            .publish(UserReadyTopic("user-42".into()), UserReady("done".into()))
            .unwrap();
    }
}
```

`publish(topic, item)` enqueues the publication in the service mailbox. Success means it was enqueued; delivery follows mailbox processing order.

## Subscribe and run

`address.subscribe(topic)` returns a future for the next publication of that exact key. Multiple one-time subscribers receive the same publication, then each must subscribe again for a later event:

```rust
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

Subscriptions also enter the service mailbox. Calling `subscribe` before sending the message that triggers publication places those operations in order. Subscribers receive only publications after registration; earlier events are not replayed.

For an ongoing subscription, use `subscribe_all`. Its `TopicAllHandle<T>` supports repeated `recv().await` calls, implements `Stream`, and has a `close()` method:

```rust,ignore
let mut events = address.subscribe_all(UserReadyTopic("user-42".into()))?;
while let Some(event) = events.recv().await {
    println!("{}", event.0);
}
```

See the complete [`topic` example](../serviceless/examples/topic.rs). For service and message macros, see the [Service guide](service.md). To expose a topic to JSON-RPC clients, see the [`jsonrpc.topic` guide](jsonrpc-topic.md).
