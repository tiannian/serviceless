# JSON-RPC Topic 通知

[English](jsonrpc-notification.md)

本文描述本仓库当前的实现。JSON-RPC 2.0 中的 **Notification（通知）** 是没有 `id` 的请求，接收方不返回响应。下文的 `rpc.subscribe` 是本项目约定的订阅方法，并非 JSON-RPC 2.0 自带的方法。

## 当前机制

`Router::topic::<T>(name, address)` 把 actor 的 `Topic` 暴露为一个具名事件源。订阅参数会直接反序列化为 `T`，也就是 **Topic 键**。Actor 发布事件时指定一个键，只有订阅了相同键的连接会收到事件。键需要实现 `Deserialize` 和 `Topic` 要求的 trait，事件项需要实现 `Serialize`。

桥接层使用 `ServiceAddress::subscribe_all`，所以一次订阅会持续收到此后发布到该键的事件。普通 `ServiceAddress::subscribe` 只等待下一次发布；如果收到后再重新订阅，重订阅期间可能漏事件。订阅建立前的事件也不会补发。

这里提供的是按键精确分发，不是对共享事件流执行任意条件过滤。例如键 `{ "user": "alice" }` 只会收到发布给 Alice 的事件。若每个订阅者要用不同谓词过滤同一条流，需要另外设计订阅源或在应用层分流。

## 定义和发布 Topic

完整可运行代码见 [`jsonrpc/server/examples/topic_notifications.rs`](../jsonrpc/server/examples/topic_notifications.rs)。关键类型和处理器如下：

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serviceless::{service, Context, Handler, Message, RoutedTopic, Service, Topic, TopicEndpoint};

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct UserTopic { user: String }

#[derive(Clone, Serialize)]
struct UserEvent { sequence: u64, message: String }

impl Topic for UserTopic {
    type Item = UserEvent;
}

#[derive(Default)]
struct Feed { events: TopicEndpoint<UserTopic> }

#[service]
impl Service for Feed {}

impl RoutedTopic<Feed> for UserTopic {
    fn endpoint(service: &mut Feed) -> &mut TopicEndpoint<Self> {
        &mut service.events
    }
}

struct Publish { user: String, sequence: u64 }
impl Message for Publish { type Result = (); }

#[async_trait]
impl Handler<Publish> for Feed {
    async fn handle(&mut self, msg: Publish, ctx: &mut Context<Self>) {
        ctx.publish_handle()
            .publish(
                UserTopic { user: msg.user },
                UserEvent { sequence: msg.sequence, message: "updated".into() },
            )
            .expect("feed is running");
    }
}
```

启动 actor，再在 JSON-RPC 路由器上注册这个 Topic：

```rust
use serviceless_jsonrpc::Router;
use serviceless_jsonrpc_server::Server;

let feed = Feed::default();
let (address, run) = Context::new(&feed).run(feed);
tokio::spawn(run);

let mut router = Router::new();
router.topic::<UserTopic>("user.updated", address.clone());
let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
Server::new(router).serve_tcp(listener).await?;
```

发布到 `UserTopic { user: "alice" }` 的事件会到达 Alice 的订阅者；发布到 Bob 的键会到达 Bob 的订阅者。示例中的生产者会交替发布到这两个键。

## 运行示例

```bash
cargo run -p serviceless-jsonrpc-server --example topic_notifications
```

示例监听 `127.0.0.1:3000`，每秒发布一个事件，交替发送给 `alice` 和 `bob`。

## WebSocket 订阅

连接 `ws://127.0.0.1:3000/`，发送带 `id` 的 JSON-RPC 请求：

```json
{"jsonrpc":"2.0","method":"rpc.subscribe","params":{"method":"user.updated","params":{"user":"alice"}},"id":1}
```

外层 `params.method` 是通过 `router.topic` 注册的事件名；内层 `params.params` 会反序列化为 `UserTopic` 键。成功响应返回 `true`：

```json
{"jsonrpc":"2.0","result":true,"id":1}
```

后续发布到相同键的事件会产生没有 `id` 的通知：

```json
{"jsonrpc":"2.0","method":"user.updated","params":{"sequence":3,"message":"update for alice"}}
```

`params` 是序列化后的 Topic 事件项。一个 WebSocket 连接只能建立一个订阅；再次订阅会返回错误。

关闭连接即取消订阅；不提供 `rpc.unsubscribe`。订阅应使用带 `id` 的调用，以便获知是否成功。

## 按行分隔的流

`Server::serve_stream`、`serve_stdio` 和 `serve_unix_lines` 使用与 WebSocket 相同的 `rpc.subscribe` 消息，每个流会话最多订阅一个事件。发送一个 JSON 文档并以 `\n` 结尾；服务端的每个响应或推送通知也是一行 JSON。响应和通知共用输出流，应通过 `id` 与 `method` 区分，而不是依赖消息位置。会话关闭时订阅结束。

## HTTP 与 SSE

普通 HTTP POST（路径 `/`）可以处理 JSON-RPC 调用，但不会保持订阅。持续接收服务端事件需要单独 POST `/events`，请求体包含一个事件名和订阅键。以示例服务为例：

```bash
curl -N -X POST -H 'Content-Type: application/json' -d '{"method":"user.updated","params":{"user":"alice"}}' http://127.0.0.1:3000/events
```

请求体中的 `method` 是注册的事件名；`params` 是 Topic 键 JSON。只有键类型能够从 JSON `null` 反序列化时才可省略 `params`。每个 SSE 事件的 `data` 字段包含一条 JSON-RPC 通知：

```text
data: {"jsonrpc":"2.0","method":"user.updated","params":{"sequence":3,"message":"update for alice"}}

```

每个 SSE POST 响应只有一个订阅。关闭 HTTP 响应即取消订阅。若使用 `Server::with_path("/rpc")`，SSE 路径变为 `/rpc/events`，普通 HTTP POST 仍发往 `/rpc`。重新连接会创建新的实时订阅；目前没有历史补发或 `Last-Event-ID` 恢复。

## 错误与当前限制

- 在 WebSocket 和按行流中，未知事件名返回 JSON-RPC 错误 `-32601`，无法反序列化的 Topic 键返回 `-32602`。SSE 建立失败时返回 HTTP 400。
- `rpc.subscribe` 是 WebSocket／按行流的会话方法，普通 HTTP POST 和传输无关的 `Router::handle_str` 均不处理它。`rpc.unsubscribe` 不受支持。
- 当前会话处理器只识别单条消息中的订阅方法，不处理 JSON-RPC 批量请求中的订阅方法。
- 订阅沿用 actor 当前无界的 `subscribe_all` 通道；尚未实现针对慢订阅者的背压或事件补发。
- 框架不会对 Topic 键进行授权检查，也不会对共享事件流做任意条件过滤。应用需要在开放订阅入口前决定哪些客户端可以订阅哪些键。
