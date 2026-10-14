# `jsonrpc.topic` extension: usage

[中文版](jsonrpc-notification.zh.md)

## Protocol scope

`jsonrpc.topic` is this project's topic subscription extension to JSON-RPC 2.0. It uses the no-`id` message form of a JSON-RPC Notification and extends the direction of notification delivery from the client request side to the server response side, so the server can push topic events to subscribers. The [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification) defines only client-to-server Notifications; server-pushed topic events belong to the `jsonrpc.topic` extension and are not standard JSON-RPC 2.0 Notifications. `jsonrpc.topic` names the extension; the current subscription method on WebSocket and newline streams is `rpc.subscribe`.

Run the [example server](../jsonrpc/server/examples/topic_notifications.rs) in one terminal:

```bash
cargo run -p serviceless-jsonrpc-server --example topic_notifications
```

It listens on `127.0.0.1:3000` and publishes `user.updated` events for `alice` and `bob`. The commands below subscribe to Alice's events. Replace `user.updated` with your registered topic name and `{"user":"alice"}` with its topic key. In server code, register a topic with `router.topic::<UserTopic>("user.updated", address)`.

## HTTP POST + SSE

Send one JSON subscription object to `/events`:

```bash
curl -N -X POST http://127.0.0.1:3000/events \
  -H 'Content-Type: application/json' \
  -d '{
    "method": "user.updated",
    "params": {
      "user": "alice"
    }
  }'
```

The response has `Content-Type: text/event-stream`. Each `data:` event contains one `jsonrpc.topic` topic event:

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

Stop the request to cancel the subscription. Open a separate POST response for each subscription. If the topic key accepts JSON `null`, `params` may be omitted. A GET request to `/events` returns HTTP 405; invalid subscription data returns HTTP 400.

Ordinary JSON-RPC calls use POST `/`, not `/events`:

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

With `Server::with_path("/rpc")`, use `/rpc/events` for SSE and `/rpc` for ordinary calls. The same paths work over HTTP/1.1 and HTTP/2. With `Server::serve_unix`, send the HTTP request through the configured Unix socket:

```bash
curl --unix-socket /path/to/server.sock -N -X POST http://localhost/events \
  -H 'Content-Type: application/json' \
  -d '{
    "method": "user.updated",
    "params": {
      "user": "alice"
    }
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
    "method": "user.updated",
    "params": {
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

WebSocket and newline streams return JSON-RPC error `-32601` for an unknown topic name and `-32602` for invalid topic parameters. HTTP SSE returns HTTP 400 for either case. Subscriptions receive new events only; reconnecting starts a new subscription.
