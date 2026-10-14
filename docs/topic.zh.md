# Topic：使用宏定义订阅与发布

[English](topic.md)

Topic 是带类型的发布订阅键。Topic 类型定义键和事件内容，Service 的字段保存其订阅者。

## 定义 Topic 与存储字段

派生 `Topic`，并用 `#[topic(item = ...)]` 声明事件类型：

```rust
use serviceless::Topic;

#[derive(Clone, Debug, PartialEq, Eq)]
struct UserReady(String);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserReady)]
struct UserReadyTopic(String);
```

Topic 键必须实现 `Ord + Clone + Send + 'static`，事件类型必须实现 `Clone + Send + 'static`。这里 `UserReadyTopic` 中的 `String` 是精确的键，不同用户的订阅互不影响。

在 Service 中添加 `TopicEndpoint<T>` 字段，并用 `#[topic]` 标记：

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

`TopicRoutes` 为 `UserReadyTopic` 实现 `RoutedTopic<Worker>`，将其路由到标记的字段。字段仍由 Service 自己存储和初始化。自定义运行时可以使用 `RuntimedTopicEndpoint<T, R>`，并通过 `#[service(runtime = "...")]` 选择相同的运行时。

## 从 Handler 发布事件

Handler 可以从 `Context` 获取发布句柄。下面把事件发布到 `user-42` 这个键：

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

`publish(topic, item)` 把发布操作送入 Service 邮箱。调用成功表示已入队，实际交付遵循邮箱处理顺序。

## 订阅并运行

`address.subscribe(topic)` 返回等待该精确键下一次发布的 Future。同一键的多个一次订阅者都会收到该事件；若要接收后续事件，需要重新订阅：

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

订阅操作也会进入 Service 邮箱。先调用 `subscribe`，再发送触发发布的消息，可使这两个操作按顺序进入邮箱。订阅只接收注册后的发布，不回放更早的事件。

持续订阅可使用 `subscribe_all`。返回的 `TopicAllHandle<T>` 可以反复调用 `recv().await`，也实现了 `Stream`，并提供 `close()` 方法：

```rust,ignore
let mut events = address.subscribe_all(UserReadyTopic("user-42".into()))?;
while let Some(event) = events.recv().await {
    println!("{}", event.0);
}
```

完整示例见 [`serviceless/examples/topic.rs`](../serviceless/examples/topic.rs)。Service 与消息宏的用法见 [Service 文档](service.zh.md)。如需向 JSON-RPC 客户端暴露 Topic，参见 [`jsonrpc.topic` 文档](jsonrpc-topic.zh.md)。
