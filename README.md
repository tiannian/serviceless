# Serviceless

Serviceless is a small **async actor** library for Rust, inspired by Actix-style APIs but kept
minimal: one mailbox per [`Service`], fully async handlers, and addresses for typed messaging plus
optional **topic** notifications.

The implementation of this crate does not use `unsafe`.

## Features

- **Async actors** — Each service runs a mailbox loop; `started` / `stopped` hooks and
  [`Handler::handle`](https://docs.rs/serviceless/latest/serviceless/trait.Handler.html) are
  `async` (use the [`async_trait`](https://docs.rs/async-trait) crate).
- **Typed messages** — Implement [`Message`](https://docs.rs/serviceless/latest/serviceless/trait.Message.html)
  and [`Handler`](https://docs.rs/serviceless/latest/serviceless/trait.Handler.html) for your own
  types instead of manual routing tables.
- **`call` and `send`** — [`ServiceAddress::call`](https://docs.rs/serviceless/latest/serviceless/struct.ServiceAddress.html#method.call)
  awaits `M::Result`; [`send`](https://docs.rs/serviceless/latest/serviceless/struct.ServiceAddress.html#method.send)
  enqueues work and drops the handler return value.
- **Topics (pub/sub-style)** — [`Topic`](https://docs.rs/serviceless/latest/serviceless/trait.Topic.html),
  [`RoutedTopic`](https://docs.rs/serviceless/latest/serviceless/trait.RoutedTopic.html), and
  [`TopicEndpoint`](https://docs.rs/serviceless/latest/serviceless/struct.TopicEndpoint.html) for
  one-shot subscribe / publish flows, still serialized through the actor mailbox.
- **External envelope streams** — [`Context::with_stream`](https://docs.rs/serviceless/latest/serviceless/struct.Context.html#method.with_stream)
  merges another stream of [`Envelope`](https://docs.rs/serviceless/latest/serviceless/struct.Envelope.html)s
  with the internal mailbox.
- **Typed narrowing** — [`ServiceAddress::into_address`](https://docs.rs/serviceless/latest/serviceless/struct.ServiceAddress.html#method.into_address)
  builds a single-message-type [`Address`](https://docs.rs/serviceless/latest/serviceless/struct.Address.html)
  plus a forwarding future you spawn next to the main run future.
- **Bring your own runtime** — The library returns a `run` future; you spawn it (examples use Tokio).
  There are **no optional Cargo `[features]`** on this crate: the full API is always available.

## Documentation

- Run **`cargo doc --open -p serviceless`** for full API reference.
- Narrative guide (actor usage, caveats, pub/sub): see the **`serviceless::docs`** module in the
  generated docs (overview, services, messaging, pub/sub, runtime).

## Usage

### Service

A `Service` is your actor type. You must set the associated `Stream` type (often
[`EmptyStream<Self>`](https://docs.rs/serviceless/latest/serviceless/type.EmptyStream.html) when
you only use the built-in mailbox).

```rust
use async_trait::async_trait;
use serviceless::{Context, EmptyStream, Service};

#[derive(Default)]
pub struct MyActor;

#[async_trait]
impl Service for MyActor {
    type Stream = EmptyStream<Self>;

    async fn started(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
        // runs once before the mailbox loop
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
        // runs after the mailbox is closed
    }
}
```

### Starting and stopping

Build a [`Context`](https://docs.rs/serviceless/latest/serviceless/struct.Context.html), start the
service, then **spawn** the returned run future on your async runtime. Until that future is polled,
mailbox work will not run.

```rust
use serviceless::Context;

# use async_trait::async_trait;
# use serviceless::{EmptyStream, Service};
# #[derive(Default)] struct MyActor;
# #[async_trait] impl Service for MyActor { type Stream = EmptyStream<Self>; }

let actor = MyActor::default();
let ctx = Context::new();
let (addr, run) = actor.start_by_context(ctx);
tokio::spawn(run);
```

Stop from inside the actor with [`Context::stop`](https://docs.rs/serviceless/latest/serviceless/struct.Context.html#method.stop),
or from outside with
[`ServiceAddress::close_service`](https://docs.rs/serviceless/latest/serviceless/struct.ServiceAddress.html#method.close_service).

### Message and handler

Declare a [`Message`](https://docs.rs/serviceless/latest/serviceless/trait.Message.html) (with
`type Result`) and implement [`Handler`](https://docs.rs/serviceless/latest/serviceless/trait.Handler.html)
for your service.

```rust
use async_trait::async_trait;
use serviceless::{Context, EmptyStream, Handler, Message, Service};

#[derive(Default)]
pub struct Service0;
#[async_trait]
impl Service for Service0 { type Stream = EmptyStream<Self>; }

pub struct U8(pub u8);

impl Message for U8 {
    type Result = U8;
}

#[async_trait]
impl Handler<U8> for Service0 {
    async fn handle(&mut self, message: U8, _ctx: &mut Context<Self, Self::Stream>) -> U8 {
        U8(message.0 + 2)
    }
}
```

### Address: `call`, `send`, and topics

[`ServiceAddress`](https://docs.rs/serviceless/latest/serviceless/struct.ServiceAddress.html) is
cloneable and is how other tasks talk to the actor.

- **`call`** — `async`; waits for `M::Result`. If the service has stopped, you get
  [`Error::ServiceStoped`](https://docs.rs/serviceless/latest/serviceless/enum.Error.html#variant.ServiceStoped).
- **`send`** — synchronous for the caller; still returns `Result` and drops the handler return value.
- **Preferred dispatch** — set `Message::IS_PERFERRED = true` when a `call` should route through
  [`Handler::handle_preferred`](https://docs.rs/serviceless/latest/serviceless/trait.Handler.html#method.handle_preferred)
  (e.g. to use [`ReplyHandle`](https://docs.rs/serviceless/latest/serviceless/struct.ReplyHandle.html)
  manually). `send` continues to route through `handle` because it has no reply channel.
  `handle_preferred` can spawn a task and reply later so the current handler path does not block
  mailbox progress (see `actor/examples/preferred.rs`).
- **`subscribe`** — `async`; registers for the next topic publication (see `serviceless::docs::pubsub`
  and `examples/topic.rs`).

## Examples

Runnable examples live under `actor/examples/` (e.g. `topic.rs`, `single.rs`, `external_stream.rs`,
`preferred.rs`).

```bash
cargo run -p serviceless --example topic
```
