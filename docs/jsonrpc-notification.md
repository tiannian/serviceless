# JSON-RPC topic notifications

[中文版](jsonrpc-notification.zh.md)

This guide describes the implementation in this repository. JSON-RPC 2.0 defines a **notification** as a request without an `id`: its receiver sends no response. The `rpc.subscribe` and `rpc.unsubscribe` methods described here are this project's subscription convention. They are not methods defined by JSON-RPC 2.0 itself.

## What the current implementation does

`Router::topic::<T>(name, address)` exposes an actor `Topic` as a named event source. The subscription parameters are deserialized directly into `T`, the **topic key**. The actor publishes an item to a key, and subscribers of that exact key receive it. The item must implement `Serialize`; the key must implement `Deserialize` as well as the requirements of `Topic`.

The bridge uses `ServiceAddress::subscribe_all`, so one subscription receives every subsequent publication for its key. `ServiceAddress::subscribe` waits for only the next publication and is unsuitable for this continuous stream unless a new one-shot subscription is made after every item. Publications before registration are not replayed.

This is exact-key routing, not a general per-subscriber filter. For example, a key `{ "user": "alice" }` receives publications addressed to Alice. Arbitrary predicates over a shared event stream need a different subscription source or additional application routing.

## Define and publish a topic

The complete runnable version is [`jsonrpc/server/examples/topic_notifications.rs`](../jsonrpc/server/examples/topic_notifications.rs). Its essential types and handler are:

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

Create and run the actor, then register its topic on the JSON-RPC router:

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

Publishing to `UserTopic { user: "alice" }` reaches Alice's subscribers; publishing to Bob's key reaches Bob's subscribers. The example starts a producer that alternates between those keys.

## Run the example

```bash
cargo run -p serviceless-jsonrpc-server --example topic_notifications
```

It listens on `127.0.0.1:3000` and publishes one event per second, alternating between `alice` and `bob`.

## WebSocket subscription

Connect to `ws://127.0.0.1:3000/` and send a JSON-RPC request containing an `id`:

```json
{"jsonrpc":"2.0","method":"rpc.subscribe","params":{"method":"user.updated","params":{"user":"alice"}},"id":1}
```

The outer `params.method` is the name registered with `router.topic`. The inner `params.params` is deserialized into the `UserTopic` key. A successful response returns a numeric subscription ID **local to this connection**:

```json
{"jsonrpc":"2.0","result":1,"id":1}
```

Each matching publication produces a notification, without an `id`:

```json
{"jsonrpc":"2.0","method":"user.updated","params":{"subscription":1,"value":{"sequence":3,"message":"update for alice"}}}
```

The `subscription` field identifies which subscription received the event; `value` is the serialized topic item. Multiple subscriptions can share one WebSocket connection. The request `id` and the returned subscription ID are different things.

To cancel, send:

```json
{"jsonrpc":"2.0","method":"rpc.unsubscribe","params":{"subscription":1},"id":2}
```

The response has `result: true` if that subscription existed on this connection, or `false` otherwise. Closing the connection cancels all its subscriptions. Use calls with an `id` for subscribe/unsubscribe; a call without `id` is a JSON-RPC notification and cannot return a subscription ID.

## Newline-framed streams

`Server::serve_stream`, `serve_stdio`, and `serve_unix_lines` use the same `rpc.subscribe` and `rpc.unsubscribe` messages as WebSocket. Send one JSON document followed by `\n`. The server writes each response or pushed notification as one JSON document followed by `\n`. Responses and notifications share the output stream, so distinguish them by `id` versus `method`, rather than by their position. The subscription belongs to that stream session and ends when the session closes.

## HTTP and SSE

Ordinary HTTP POST at `/` handles JSON-RPC calls but does not keep a subscription open. Use a separate GET request to `/events` for a server-sent event stream. With the example server:

```bash
curl -N 'http://127.0.0.1:3000/events?method=user.updated&params=%7B%22user%22%3A%22alice%22%7D'
```

The `method` query value is the registered event name. The `params` query value is URL-encoded JSON for the topic key; omit it only when the key type can deserialize from JSON `null`. Each SSE event contains a JSON-RPC notification in its `data` field:

```text
data: {"jsonrpc":"2.0","method":"user.updated","params":{"sequence":3,"message":"update for alice"}}

```

SSE uses one subscription per GET response, so these notifications do not include the WebSocket/stream `subscription` wrapper. Close the HTTP response to unsubscribe. If `Server::with_path("/rpc")` is used, the SSE path is `/rpc/events`. HTTP POST calls still go to `/rpc`. Reconnecting creates a new live subscription; there is no event replay or `Last-Event-ID` recovery.

## Errors and current limits

- On WebSocket and newline streams, an unknown event name returns JSON-RPC error `-32601`; invalid topic-key JSON returns `-32602`. SSE setup failures return HTTP 400.
- `rpc.subscribe` and `rpc.unsubscribe` are session methods on WebSocket/newline streams. They are not available through ordinary HTTP POST or through the transport-independent `Router::handle_str` API.
- The current session handler recognizes these methods as individual messages, not as entries in a JSON-RPC batch.
- Subscription storage uses the actor's current unbounded `subscribe_all` channel. This implementation does not apply per-subscriber backpressure or replay.
- The framework does not perform authorization or arbitrary filtering for a key. Applications must decide which keys a client may subscribe to before exposing the endpoint.
