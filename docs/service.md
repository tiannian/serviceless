# Services and messages with macros

[中文](service.zh.md)

This guide uses the current `serviceless` facade API. A service processes typed messages through one mailbox. A callable operation consists of a message type and the service's `Handler` implementation for that type.

## Define and run a service

```rust
use async_trait::async_trait;
use serviceless::{service, Context, Handler, Message, Service};

struct Counter(u32);

#[service(name = "counter")]
impl Service for Counter {}

#[derive(Message)]
#[message(result = u32)]
struct Add(u32);

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, message: Add, _ctx: &mut Context<Self>) -> u32 {
        self.0 += message.0;
        self.0
    }
}

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

Put `#[service]` on an `impl Service for Type` block. It supplies `type Runtime = TokioRuntime`, `type Stream = EmptyStream<Self>`, and a metadata name taken from the type name. Use `#[service(name = "counter", runtime = "path::CustomRuntime")]` to override the name or runtime. The `runtime` value is a quoted Rust type path. An explicit `type Stream` or `metadata()` method is preserved. Do not combine `name` with an explicit `metadata()` method, or define `Runtime` inside the impl when using the macro.

The macro also applies `async_trait` to the Service implementation, so lifecycle hooks can be written directly in that block:

```rust,ignore
#[service]
impl Service for Counter {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        // Called before processing the mailbox.
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        // Called when the service processing loop exits.
    }
}
```

## Define messages and handlers

`#[derive(Message)]` together with `#[message(result = Type)]` generates `Message::Result`. Messages can be structs or enums, including generic types. A service can implement `Handler<M>` for multiple message types.

Handler implementations still need `#[async_trait]`. The `handle` method receives the message and the service's `Context`, then returns the declared result type. `address.call(message).await` waits for that result. `address.send(message)` only enqueues the message and does not wait for handling. Delivery can fail if the service has stopped. Messages and results must satisfy the call API's `Send` bounds.

For a deferred reply, override `Handler::handle_preferred` and send the result through `ReplyHandle<M>`. In the usual case, implement only `handle`; the default `handle_preferred` calls it and replies.

`Context::new(&service).run(service)` returns an address and a future. Spawn that future on the async runtime to run the service. To add a stream of `Envelope` values, declare `type Stream` explicitly and use `Context::with_stream`.

See the complete [`service_macro` example](../serviceless/examples/service_macro.rs). For publishing and subscribing, see the [Topic guide](topic.md).
