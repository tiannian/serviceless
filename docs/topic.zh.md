# Topic：使用宏定义订阅与发布

[English](topic.md)

Topic 是带类型的发布订阅键。Topic 类型决定键和事件内容的类型；Service 上的 `TopicEndpoint<T>` 字段保存该 Topic 的订阅者；`#[derive(TopicRoutes)]` 将二者连接起来。

## 完整示例

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

`#[derive(Topic)]` 配合 `#[topic(item = 事件类型)]` 生成 `Topic::Item`。Topic 键必须实现 `Ord + Clone + Send + 'static`，事件类型必须实现 `Clone + Send + 'static`。上例中 `UserReadyTopic("user-42")` 是订阅和发布时使用的精确键；不同键分别维护订阅者。

在具名字段的 Service 结构体上派生 `TopicRoutes`，并给每个 Topic 的 `TopicEndpoint<T>` 字段标上 `#[topic]`。该宏为 `T` 实现 `RoutedTopic<Worker>`，路由到对应字段。字段仍由 Service 自己存储和初始化，可通过 `Default` 初始化。自定义运行时可使用 `RuntimedTopicEndpoint<T, R>`；Service 的 `#[service(runtime = "...")]` 应选择相同的运行时。

## 一次订阅与持续订阅

`address.subscribe(topic)` 返回一个等待该键下一次发布的 Future。同一键的多个一次订阅者都会收到那次发布；收到后需重新订阅。`address.subscribe_all(topic)` 返回 `TopicAllHandle<T>`，可反复调用 `recv().await` 接收后续事件，也可作为 Stream 使用，或调用 `close()` 结束接收。

```rust,ignore
let mut events = address.subscribe_all(UserReadyTopic("user-42".into()))?;
while let Some(event) = events.recv().await {
    println!("{}", event.0);
}
```

发布使用 `ctx.publish_handle().publish(topic, item)`。它将发布操作送入同一 Service 邮箱，交付顺序由邮箱处理顺序决定；调用成功表示操作已入队。订阅同样先入队，因此先调用 `subscribe` 或 `subscribe_all`、再发送触发发布的消息，可以确保这些操作按发送顺序进入同一邮箱。订阅只接收注册后的发布，不回放历史事件。`publish` 返回投递错误时通常意味着 Service 已停止。

完整示例见 [`serviceless/examples/topic.rs`](../serviceless/examples/topic.rs)。Service 与消息宏的用法见 [Service 文档](service.zh.md)。如果要把 Topic 暴露给 JSON-RPC 客户端，参见 [`jsonrpc.topic` 文档](jsonrpc-topic.zh.md)。
