# JSON-RPC examples

Run these commands from the repository root. Each server starts with fresh state;
HTTP and WebSocket connections to the counter server share the same actor.

## Counter over HTTP

```sh
cargo run -p serviceless-jsonrpc-server --features tcp --example counter
```

In another terminal, call the actor:

```sh
curl http://127.0.0.1:3000/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2},"id":1}'
```

The result is `{"jsonrpc":"2.0","result":2,"id":1}` (key order may vary).
Send a notification by omitting `id`; HTTP returns 204 with an empty body:

```sh
curl -i http://127.0.0.1:3000/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"counter.add","params":{"amount":3}}'
```

Read the state and trigger a business error in one batch:

```sh
curl http://127.0.0.1:3000/ -H 'Content-Type: application/json' \
  -d '[{"jsonrpc":"2.0","method":"counter.get","params":{},"id":2},{"jsonrpc":"2.0","method":"counter.add","params":{"amount":9223372036854775807},"id":3}]'
```

After the requests above, the first response has `result: 5`; the second has
`error: {"code":1001,"message":"Counter overflow"}`. The counter remains 5.
Unknown methods return -32601; invalid message parameters return -32602.

## Counter over WebSocket

Keep the counter HTTP server running and start the client in another terminal:

```sh
cargo run -p serviceless-jsonrpc-server --features tcp --example websocket_client
```

The client sends a notification followed by a batch containing an add and a get.
On a fresh server both batch results are 5. Each client run adds 5 to shared state.

## Counter over stdio

Each input line contains one JSON document. Only calls produce output lines.
This batch sends a notification and reads the resulting state:

```sh
printf '%s\n' '[{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}},{"jsonrpc":"2.0","method":"counter.get","params":{},"id":1}]' \
  | cargo run --quiet -p serviceless-jsonrpc-server --example counter -- stdio
```

Output: `[{"jsonrpc":"2.0","result":2,"id":1}]` (key order may vary).
The server exits when stdin reaches EOF.

## Topic notifications over SSE

Stop the counter server before starting the topic server on the same port:

```sh
cargo run -p serviceless-jsonrpc-server --features tcp --example topic_notifications
```

Subscribe to Alice's updates in another terminal:

```sh
curl -N http://127.0.0.1:3000/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"user.updated","topic":{"user":"alice"}},"id":1}'
```

The SSE stream first acknowledges the subscription, then delivers `user.updated`
notifications for Alice. Change the topic's `user` to `bob` for Bob's updates.
See the [protocol guide](../../../docs/jsonrpc.md) and
[topic guide](../../../docs/jsonrpc-topic.md) for more details.

Stream and stdio are available by default. Enable Cargo features `tcp`, `unix`, or `unixlines` for `serve_tcp`, `serve_unix`, or `serve_unix_lines`, respectively. Unix sockets require a Unix platform. For example: `serviceless-jsonrpc-server = { version = "0.7.3", features = ["tcp"] }`.
