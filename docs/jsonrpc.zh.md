# JSON-RPC 协议行为

[English](jsonrpc.md)

`serviceless-jsonrpc` 将 Serviceless 的 Actor 模型直接对接到 JSON-RPC 2.0 协议。注册后的 JSON-RPC 方法通过 Actor 地址调用现有的 Handler：协议中的 `method` 选择 Actor 消息类型，`params` 提供消息数据，Handler 的结果转换为响应。带 `id` 的请求使用 `address.call`，不带 `id` 的 Notification 使用 `address.send`。

## 方法映射

每个已注册的 JSON-RPC 方法名映射到一种有类型的 Actor 消息。Router 将请求中的 `params` 反序列化为该消息。调用时，Handler 的成功结果转换为 JSON-RPC `result`；Handler 错误转换为 JSON-RPC 错误。

注册方法并将 Router 交给传输服务：

```rust,ignore
let mut router = serviceless_jsonrpc::Router::new();
router.method::<Add>("counter.add", address.clone());
let server = serviceless_jsonrpc_server::Server::new(router);
```

其中 `Add` 实现 `Message` 和 `serde::Deserialize`，结果类型是 `Result<R, E>`，且 `R: serde::Serialize`、`E: Into<JsonRpcError>`。

## 调用与 Notification

带 `id` 的请求通过 `address.call` 调用 Actor，并等待 Handler 的结果：

```json
{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2},"id":1}
```

如果 Handler 返回 `Ok(2)`，响应是：

```json
{"jsonrpc":"2.0","result":2,"id":1}
```

省略 `id` 即发送 JSON-RPC Notification。Router 通过 `address.send` 将消息放入 Actor 邮箱，不等待 Handler 的结果，也不发送响应：

```json
{"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}}
```

显式写入 `"id": null` 仍是调用，而不是 Notification；响应中的 `id` 也是 `null`。即使方法不存在、参数无效或 Handler 执行失败，Notification 也没有响应。成功放入 Actor 邮箱不代表 Handler 已成功执行。

`params` 可以是对象或数组，具体由消息的反序列化实现决定。省略 `params` 时，反序列化器收到 JSON `null`。无效请求返回 `-32600`，未知方法返回 `-32601`，消息参数无效返回 `-32602`。Handler 错误经 `JsonRpcError` 转换后返回对应的错误码、消息和可选数据。对于调用，`Err(())` 和 Actor 投递失败返回 `-32603`。

## 批量请求

JSON 数组中的条目按输入顺序处理。响应汇总为数组，Notification 不占响应条目。全部由 Notification 组成的批次没有响应；空数组返回 Invalid Request 错误。

```json
[
  {"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}},
  {"jsonrpc":"2.0","method":"counter.add","params":{"amount":3},"id":"next"}
]
```

响应只包含第二个条目。若计数器初始值为零，响应为：

```json
[{"jsonrpc":"2.0","result":5,"id":"next"}]
```

## 传输方式

HTTP POST 和 WebSocket 默认使用 `/`，也可以使用配置的端点路径。调用或 Notification 都放在 POST 请求体中；HTTP 对调用返回 JSON，对 Notification 返回 204 和空响应体。WebSocket 每条文本消息承载一份 JSON 文档；按行分隔的流每行承载一份 JSON 文档。Notification 在这两种传输中也不产生响应消息。在支持的传输方式中，TCP、Unix socket 和标准输入输出遵循相同的 JSON-RPC 行为。

服务端推送的 Topic 事件和订阅参见 [`jsonrpc.topic` 指南](jsonrpc-topic.zh.md)。

默认提供 stream 和 stdio。启用 Cargo feature `tcp`、`unix`、`unixlines` 可分别使用 `serve_tcp`、`serve_unix`、`serve_unix_lines`；Unix socket 仅支持 Unix 平台。例如：`serviceless-jsonrpc-server = { version = "0.7.2", features = ["tcp"] }`。
