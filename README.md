# Serviceless

Serviceless is a small async actor library for Rust. A service processes typed messages through
one mailbox, supports async handlers and lifecycle hooks, and can publish topic events.

## Crates

| Crate | Purpose |
| --- | --- |
| `serviceless-actor` (`actor/`) | Runtime-independent actor traits, context, addresses, messages, and topics. |
| `serviceless-tokio` (`tokio/`) | Tokio implementations of the channel, task, and runtime traits. |
| `serviceless` (`serviceless/`) | Convenient re-exports and Tokio-specific type aliases. |
| `serviceless-jsonrpc` (`jsonrpc/core/`) | Transport-independent JSON-RPC 2.0 method routing to typed actor messages. |
| `serviceless-jsonrpc-server` (`jsonrpc/server/`) | Hyper HTTP/1.1 and HTTP/2, WebSocket, Unix socket, and stdio transports. |

The `serviceless-actor` implementation does not use `unsafe`. Its run loop uses futures rather
than Tokio APIs, so another runtime can implement the traits in `serviceless_actor::runtime`.

## Example

```rust
use async_trait::async_trait;
use serviceless::{service, Context, Handler, Message, Service};

struct Counter(u32);

#[service(name = "counter")]
impl Service for Counter {
}

struct Add(u32);
impl Message for Add {
    type Result = u32;
}

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, message: Add, _ctx: &mut Context<Self>) -> u32 {
        self.0 += message.0;
        self.0
    }
}

#[tokio::main]
async fn main() {
    let service = Counter(0);
    let (address, run) = Context::new(&service).run(service);
    let task = tokio::spawn(run);

    assert_eq!(address.call(Add(2)).await.unwrap(), 2);
    address.close_service();
    task.await.unwrap();
}
```

The `Service` name in `serviceless` refers to the core `RuntimedService` trait. The
`#[service]` attribute supplies `TokioRuntime`, `EmptyStream<Self>`, and a metadata name based on
the type name by default. Use `#[service(name = "counter", runtime = "path::CustomRuntime")]`
to override either value. An explicit `type Stream` or `metadata()` method is kept; `name` and an
explicit `metadata()` method cannot be used together. Handler implementations still use
`#[async_trait]`.

The facade also exports `ReplyHandle`, `TopicEndpoint`, and `TopicAllHandle` aliases for Tokio.
`Message` and `Topic` can derive their associated types, and `TopicRoutes` generates routing
from explicitly stored endpoint fields:

```rust,ignore
#[derive(Message)]
#[message(result = u32)]
struct Add(u32);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserReady)]
struct UserReadyTopic(String);

#[derive(Default, TopicRoutes)]
struct MyService {
    #[topic]
    user_ready: TopicEndpoint<UserReadyTopic>,
}

#[service]
impl Service for MyService {}
```

`#[topic]` on a field generates `RoutedTopic<MyService>` for the topic named by its
`TopicEndpoint<T>` type. The endpoint remains an ordinary field, so service construction is
explicit. `TopicRoutes` also accepts `RuntimedTopicEndpoint<T, R>` for custom runtimes.

More examples are in `serviceless/examples/`. For example:

```bash
cargo run -p serviceless --example service_macro
cargo run -p serviceless --example topic
```

## JSON-RPC routing

`serviceless-jsonrpc::Router` connects an exact method name to an actor message. The message
must implement `serde::Deserialize`, and its `Message::Result` must be `Result<R, E>` with
`R: serde::Serialize` and `E: Into<JsonRpcError>`. Use `std::convert::Infallible` or `()` as the
error type for methods without business errors. The router decodes `params`, calls the actor,
and serializes `Ok(R)` as the JSON-RPC result. It accepts a JSON string
with `handle_str` or a `serde_json::Value` with `handle_value`; both return `None` for notifications.
It does not provide an HTTP, WebSocket, or stdio transport.

```rust,ignore
let mut router = serviceless_jsonrpc::Router::new();
router.method::<Add>("counter.add", address.clone());

let response: Option<String> = router
    .handle_str(r#"{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2},"id":1}"#)
    .await;
```

Methods are registered explicitly; an actor can expose any subset of its handlers. Registration
of the same name again replaces the previous method. Batch entries are processed in input order.
An `Err(E)` becomes a JSON-RPC error. `JsonRpcError::new(code, message).with_data(value)` adds
optional error data. `Err(())` and actor delivery failures become internal errors. Notifications
never receive an error response.

See the [English JSON-RPC guide](docs/jsonrpc.md) or [中文用法指南](docs/jsonrpc.zh.md)
for the protocol behavior of calls, notifications, batches, and transports.

## JSON-RPC server

`serviceless-jsonrpc-server::Server` takes a configured `Router`. Bind a TCP listener and pass it
to `serve_tcp` for HTTP POST and WebSocket requests at `/`. The Hyper connection accepts HTTP/1.1
and cleartext HTTP/2; WebSocket uses HTTP/1.1 Upgrade. A notification sent over HTTP receives
status 204 with no body. Use `with_path` to change the endpoint and `with_max_message_size` to
set the per-request size limit (default: 1 MiB).

```rust,ignore
let server = serviceless_jsonrpc_server::Server::new(router).with_path("/rpc");
let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
server.serve_tcp(listener).await?;
```

### `jsonrpc.topic` extension

Server-pushed topic events use this project's `jsonrpc.topic` extension. It extends the
no-`id` JSON-RPC Notification form from the client request side to the server response side
to support topic subscriptions. See the guides for the protocol scope.

See the [English usage guide](docs/jsonrpc-topic.md) or
[中文用法指南](docs/jsonrpc-topic.zh.md) for HTTP POST + SSE, WebSocket,
stdio, and newline-framed Unix socket requests. Run the
[example server](jsonrpc/server/examples/topic_notifications.rs) with
`cargo run -p serviceless-jsonrpc-server --example topic_notifications`.

On Unix, `serve_unix` serves HTTP and WebSocket over a Unix socket, while `serve_unix_lines`
serves newline-delimited JSON-RPC directly on the socket. `serve_stdio` uses the same one-JSON-
document-per-line framing on stdin and stdout. These functions run until the listener or input
closes; the caller owns shutdown and Unix socket path cleanup.
