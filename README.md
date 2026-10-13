# Serviceless

Serviceless is a small async actor library for Rust. A service processes typed messages through
one mailbox, supports async handlers and lifecycle hooks, and can publish topic notifications.

## Crates

| Crate | Purpose |
| --- | --- |
| `serviceless-actor` (`actor/`) | Runtime-independent actor traits, context, addresses, messages, and topics. |
| `serviceless-tokio` (`tokio/`) | Tokio implementations of the channel, task, and runtime traits. |
| `serviceless` (`serviceless/`) | Convenient re-exports and Tokio-specific type aliases. |
| `service-channel` (`channel/`) | Standalone channel crate; the actor crates do not depend on it. |

The `serviceless-actor` implementation does not use `unsafe`. Its run loop uses futures rather
than Tokio APIs, so another runtime can implement the traits in `serviceless_actor::runtime`.

## Example

```rust
use async_trait::async_trait;
use serviceless::{Context, EmptyStream, Handler, Message, Metadata, Service, TokioRuntime};

struct Counter(u32);

#[async_trait]
impl Service for Counter {
    type Stream = EmptyStream<Self>;
    type Runtime = TokioRuntime;

    fn metadata(&self) -> Metadata<'_> {
        Metadata { name: "counter" }
    }
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

The `Service` name in `serviceless` refers to the core `RuntimedService` trait. Set its
`Runtime` associated type to `TokioRuntime` when using the Tokio adapter. The facade also exports
`ReplyHandle`, `TopicEndpoint`, and `TopicAllHandle` aliases for that runtime.

More examples are in `serviceless/examples/`. For example:

```bash
cargo run -p serviceless --example topic
```
