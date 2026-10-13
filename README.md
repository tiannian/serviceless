# Serviceless

Serviceless is a small async actor library for Rust. A service processes typed messages through
one mailbox, supports async handlers and lifecycle hooks, and can publish topic notifications.

## Crates

| Crate | Purpose |
| --- | --- |
| `serviceless-actor` (`actor/`) | Runtime-independent actor traits, context, addresses, messages, and topics. |
| `serviceless-tokio` (`tokio/`) | Tokio implementations of the channel, task, and runtime traits. |
| `serviceless` (`serviceless/`) | Convenient re-exports and Tokio-specific type aliases. |
| `serviceless-jsonrpc` (`jsonrpc/`) | Transport-independent JSON-RPC 2.0 method routing to typed actor messages. |

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
