# Services and messages with macros

[中文](service.zh.md)

A service processes typed messages through one mailbox. A callable operation consists of a message type and a `Handler` implementation for that type.

## Define a service

Put `#[service]` on an `impl Service for Type` block:

```rust
use serviceless::{service, Service};

struct Counter(u32);

#[service(name = "counter")]
impl Service for Counter {}
```

The macro supplies `type Runtime = TokioRuntime` and `type Stream = EmptyStream<Self>`. Without `name`, the metadata name is the type name. To select another runtime, use a quoted Rust type path such as `#[service(runtime = "path::CustomRuntime")]`. An explicit `type Stream` or `metadata()` method is preserved. Do not combine `name` with `metadata()`, or define `Runtime` inside the impl when using the macro.

The macro also applies `async_trait` to this impl. Lifecycle hooks can therefore be written directly in it:

```rust,ignore
#[service]
impl Service for Counter {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        // Called before processing the mailbox.
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        // Called when the processing loop exits.
    }
}
```

## Define a message and its handler

`#[derive(Message)]` with `#[message(result = Type)]` generates `Message::Result`. Messages can be structs or enums, including generic types:

```rust
use serviceless::Message;

#[derive(Message)]
#[message(result = u32)]
struct Add(u32);
```

Implement `Handler<Add>` to define what the service does with that message. Handler implementations still need `#[async_trait]`:

```rust
use async_trait::async_trait;
use serviceless::{Context, Handler};

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, message: Add, _ctx: &mut Context<Self>) -> u32 {
        self.0 += message.0;
        self.0
    }
}
```

A service can implement `Handler<M>` for multiple message types. For a deferred reply, override `handle_preferred` and send the result through `ReplyHandle<M>`. In the usual case, implement only `handle`; the default `handle_preferred` calls it and replies.

## Run and call the service

`Context::new(&service).run(service)` returns an address and a future. Spawn that future on the async runtime:

```rust
#[tokio::main]
async fn main() {
    let counter = Counter(0);
    let (address, run) = Context::new(&counter).run(counter);
    let task = tokio::spawn(run);

    assert_eq!(address.call(Add(2)).await.unwrap(), 2);
    address.send(Add(3)).unwrap();

    address.close_service();
    task.await.unwrap();
}
```

`call(...).await` waits for the result. `send(...)` only enqueues the message. Delivery can fail if the service has stopped; messages and results must satisfy the call API's `Send` bounds. To add a stream of `Envelope` values, declare `type Stream` explicitly and use `Context::with_stream`.

See the complete [`service_macro` example](../serviceless/examples/service_macro.rs). For publishing and subscribing, see the [Topic guide](topic.md).
