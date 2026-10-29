# JSON-RPC protocol behavior

[中文版](jsonrpc.zh.md)

`serviceless-jsonrpc` directly connects the Serviceless actor model to JSON-RPC 2.0. A registered JSON-RPC method targets an existing actor handler through its address: the protocol's `method` selects the actor message type, `params` supplies the message data, and the handler result becomes the response. A request with `id` uses `address.call`; a notification without `id` uses `address.send`.

## Method mapping

Each registered JSON-RPC method name maps to one typed actor message. The router deserializes the request's `params` into that message. A call converts the handler's successful result into JSON-RPC `result`; a handler error becomes a JSON-RPC error.

Register a method and pass the router to the transport server:

```rust,ignore
let mut router = serviceless_jsonrpc::Router::new();
router.method::<Add>("counter.add", address.clone());
let server = serviceless_jsonrpc_server::Server::new(router);
```

Here `Add` implements `Message` and `serde::Deserialize`, and its result is `Result<R, E>` with `R: serde::Serialize` and `E: Into<JsonRpcError>`.

Configure typed parameter decoding, handler result encoding, and topic events with
`Router::new().with_config(serviceless_jsonrpc::Config::default().set_bytes_hex().enable_hex_prefix())`.
`Router::new()` and `Router::default()` use `serde_json_ext::Config::default()`.
The configuration applies to all registered methods and topics. Byte fields must use
`#[serde(with = "serde_bytes")]` (or an equivalent bytes serializer) for byte encoding options.
JSON-RPC envelopes and prebuilt `serde_json::Value` values retain their JSON representation.
Custom `RpcResult` or `RpcTopicAddress` implementations should override their
`*_with_config` method to support the router configuration.

## Call and notification

A request with an `id` calls the actor through `address.call` and waits for the handler result:

```json
{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2},"id":1}
```

For a handler returning `Ok(2)`, the response is:

```json
{"jsonrpc":"2.0","result":2,"id":1}
```

Omit `id` to send a JSON-RPC notification. The router uses `address.send` to enqueue the message without waiting for the handler result, and sends no response:

```json
{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}}
```

An explicit `"id": null` is a call, not a notification. Its response has `"id": null`. A notification receives no response even when its method is unknown, its parameters are invalid, or the handler fails. Successful delivery to the actor mailbox does not confirm that the handler completed successfully.

`params` may be an object or array, according to the message's deserializer. Omitting `params` passes JSON `null` to the deserializer. An invalid request returns `-32600`; an unknown method returns `-32601`; invalid message parameters return `-32602`. A handler error converted into `JsonRpcError` supplies its code, message, and optional data. `Err(())` and actor delivery failures return `-32603` for calls.

## Batches

A JSON array is processed in input order. Responses are collected in an array; notifications have no entries. A batch containing only notifications produces no response. An empty batch returns an Invalid Request error.

```json
[
  {"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}},
  {"jsonrpc":"2.0","method":"counter.add","params":{"amount":3},"id":"next"}
]
```

The response contains only the second entry. For a counter starting at zero, it is:

```json
[{"jsonrpc":"2.0","result":5,"id":"next"}]
```

## Transports

HTTP POST and WebSocket use `/` by default, or a configured endpoint path. Send a call or notification in the POST body. HTTP returns JSON for a call and status 204 with an empty body for a notification. WebSocket accepts one JSON document per text message; newline streams accept one JSON document per line. Neither sends a message for a notification. The same JSON-RPC behavior applies over TCP, Unix sockets, and standard input/output where those transports are available.

For server-pushed topic events and subscriptions, see the [`jsonrpc.topic` guide](jsonrpc-topic.md).

Stream and stdio are available by default. Enable Cargo features `tcp`, `unix`, or `unixlines` for `serve_tcp`, `serve_unix`, or `serve_unix_lines`, respectively. Unix sockets require a Unix platform. For example: `serviceless-jsonrpc-server = { version = "0.7.2", features = ["tcp"] }`.
