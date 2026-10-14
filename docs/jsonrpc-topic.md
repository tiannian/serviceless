# `jsonrpc.topic` extension: usage

[中文版](jsonrpc-topic.zh.md)

## Protocol scope

`jsonrpc.topic` is this project's topic subscription extension to JSON-RPC 2.0. It uses the no-`id` message form of a JSON-RPC Notification and extends the direction of notification delivery from the client request side to the server response side, so the server can push topic events to subscribers. The [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification) defines only client-to-server Notifications; server-pushed topic events belong to the `jsonrpc.topic` extension and are not standard JSON-RPC 2.0 Notifications. `jsonrpc.topic` names the extension; all subscription transports use the `rpc.subscribe` method.

Every transport carries the same three JSON-RPC messages in order:

1. Client to server: an `rpc.subscribe` request with an `id`, `params.name` (registered topic name), and optional `params.topic` (topic key).
2. Server to client: a success response with the same `id` and `result: true`.
3. Server to client: zero or more event Notifications with `method` set to the topic name, `params` set to the event, and no `id`.

Only the framing differs: SSE puts the response and Notifications in successive `data:` events of one HTTP response; WebSocket sends separate text messages; newline-framed streams send separate lines. A failed subscription returns an error response instead of steps 2 and 3.

The examples below use a service at `127.0.0.1:3000` that publishes `user.updated` events for `alice` and `bob`. Replace `user.updated` with your registered topic name and `{"user":"alice"}` with its topic key.

Register the topic on the same router used for ordinary methods:

```rust,ignore
let mut router = serviceless_jsonrpc::Router::new();
router.topic::<UserTopic>("user.updated", address.clone());
let server = serviceless_jsonrpc_server::Server::new(router);
```

`UserTopic` is deserialized from `params.topic`; its published item is serialized into the event's `params`.

## HTTP POST + SSE

Send one `rpc.subscribe` JSON-RPC request to the same endpoint as ordinary calls (`/` by default):

```bash
curl -N -X POST http://127.0.0.1:3000/ \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "rpc.subscribe",
    "params": {
      "name": "user.updated",
      "topic": { "user": "alice" }
    },
    "id": 1
  }'
```

The response has `Content-Type: text/event-stream`. Its first `data:` event is the subscription result `{"jsonrpc":"2.0","result":true,"id":1}`. Subsequent `data:` events contain `jsonrpc.topic` topic events:

```json
{
  "jsonrpc": "2.0",
  "method": "user.updated",
  "params": {
    "sequence": 3,
    "message": "update for alice"
  }
}
```

This shows the JSON inside an SSE `data:` event. On the wire, the `data:` line contains the JSON on one line, followed by a blank line.

Stop the request to cancel the subscription. Open a separate POST response for each subscription. If the topic key accepts JSON `null`, the `params.topic` may be omitted. A GET request to `/` returns HTTP 405. If a subscription fails, its only SSE `data:` event contains a JSON-RPC error response with the request `id` when valid, and the stream then ends. Unknown topics use `-32601` and invalid parameters use `-32602`.

Ordinary JSON-RPC calls also use POST `/`:

```bash
curl -X POST http://127.0.0.1:3000/ \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "your.method",
    "params": {},
    "id": 1
  }'
```

With `Server::with_path("/rpc")`, both SSE subscriptions and ordinary calls use `/rpc`. The same path works over HTTP/1.1 and HTTP/2. With `Server::serve_unix`, send the HTTP request through the configured Unix socket:

```bash
curl --unix-socket /path/to/server.sock -N -X POST http://localhost/ \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "rpc.subscribe",
    "params": {
      "name": "user.updated",
      "topic": { "user": "alice" }
    },
    "id": 1
  }'
```

## WebSocket

Connect to `ws://127.0.0.1:3000/` (`/rpc` if configured with `with_path("/rpc")`). Send this JSON-RPC call as one text message:

```bash
websocat ws://127.0.0.1:3000/
```

```json
{
  "jsonrpc": "2.0",
  "method": "rpc.subscribe",
  "params": {
    "name": "user.updated",
    "topic": {
      "user": "alice"
    }
  },
  "id": 1
}
```

Successful reply:

```json
{
  "jsonrpc": "2.0",
  "result": true,
  "id": 1
}
```

Subsequent text messages contain `jsonrpc.topic` topic events:

```json
{
  "jsonrpc": "2.0",
  "method": "user.updated",
  "params": {
    "sequence": 3,
    "message": "update for alice"
  }
}
```

One WebSocket connection accepts one subscription. Close it to cancel. A second `rpc.subscribe` call on that connection returns an `Already subscribed` error. `rpc.unsubscribe` is not supported. Other JSON-RPC calls can use the same WebSocket connection.

## Newline-framed streams

`Server::serve_stdio`, `Server::serve_stream`, and `Server::serve_unix_lines` use the WebSocket JSON messages shown above: the subscription request, its reply, and subsequent topic events. Write each request followed by a newline. Read each reply or topic event as one JSON line. The formatted JSON above shows the contents of those lines.

Each stream session accepts one subscription. Close the stream to cancel. For `serve_stdio`, write to the process's stdin and read its stdout. For `serve_unix_lines`, connect to the configured Unix socket, for example with `socat - UNIX-CONNECT:/path/to/server.sock`, and send the same newline-terminated JSON. Replies have an `id`; pushed topic events have a `method` and no `id`.

## Subscription errors

All transports return JSON-RPC error `-32601` for an unknown topic name and `-32602` for invalid topic parameters. SSE sends the error in a `data:` event and ends the stream; WebSocket and newline streams send the error as a message. Malformed JSON uses `-32700`, and an invalid request uses `-32600`. Subscriptions receive new events only; reconnecting starts a new subscription.
