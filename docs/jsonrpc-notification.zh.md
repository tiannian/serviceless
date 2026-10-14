# `jsonrpc.topic` 扩展用法

[English](jsonrpc-notification.md)

## 协议范围

`jsonrpc.topic` 是本项目在 JSON-RPC 2.0 基础上定义的 Topic 订阅扩展。它沿用 JSON-RPC Notification 的无 `id` 消息形式，并将通知的发送方向从客户端请求侧扩展到服务端响应侧，以向订阅者推送 Topic 事件。[JSON-RPC 2.0 规范](https://www.jsonrpc.org/specification)只定义了客户端发送给服务端的 Notification；服务端推送的 Topic 事件属于 `jsonrpc.topic` 扩展，不是标准 JSON-RPC 2.0 Notification。`jsonrpc.topic` 是扩展名称；当前 WebSocket 和按行流使用 `rpc.subscribe` 方法发起订阅。

先在一个终端运行[示例服务](../jsonrpc/server/examples/topic_notifications.rs)：

```bash
cargo run -p serviceless-jsonrpc-server --example topic_notifications
```

服务监听 `127.0.0.1:3000`，发布 `alice` 和 `bob` 的 `user.updated` 事件。下列调用订阅 Alice 的事件。实际使用时，将 `user.updated` 换成注册的事件名，将 `{"user":"alice"}` 换成对应的 Topic 键。服务端用 `router.topic::<UserTopic>("user.updated", address)` 注册事件。

## HTTP POST + SSE

向 `/events` 发送一份 JSON 订阅信息：

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

响应的 `Content-Type` 是 `text/event-stream`。每个 `data:` 事件包含一条 `jsonrpc.topic` Topic 事件：

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

上面展示的是 SSE `data:` 事件中的 JSON 内容。实际传输时，`data:` 行中的 JSON 占一行，后面跟一个空行。

关闭请求即取消订阅。每个订阅单独打开一个 POST 响应。如果 Topic 键接受 JSON `null`，可以省略 `params`。GET `/events` 返回 HTTP 405；订阅信息无效时返回 HTTP 400。

普通 JSON-RPC 调用发送到 POST `/`，不发送到 `/events`：

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

使用 `Server::with_path("/rpc")` 时，SSE 路径是 `/rpc/events`，普通调用路径是 `/rpc`。HTTP/1.1 与 HTTP/2 使用相同的路径。使用 `Server::serve_unix` 时，可通过配置的 Unix socket 发送 HTTP 请求：

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

连接 `ws://127.0.0.1:3000/`；若设置了 `with_path("/rpc")`，则连接 `/rpc`。发送一条文本消息：

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

成功响应：

```json
{
  "jsonrpc": "2.0",
  "result": true,
  "id": 1
}
```

之后收到的 `jsonrpc.topic` Topic 事件：

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

一个 WebSocket 连接只接受一个订阅。关闭连接即取消订阅。再次调用 `rpc.subscribe` 会返回 `Already subscribed` 错误。不支持 `rpc.unsubscribe`。同一连接仍可发送其他 JSON-RPC 调用。

## 按行分隔的流

`Server::serve_stdio`、`Server::serve_stream` 和 `Server::serve_unix_lines` 使用上面 WebSocket 部分展示的 JSON 消息：订阅请求、成功响应和后续 Topic 事件。每条请求末尾加换行符；每条响应或 Topic 事件占一行。上面的多行 JSON 展示了各行的内容。

每个流会话只接受一个订阅，关闭流即取消。`serve_stdio` 从进程 stdin 接收请求、向 stdout 写响应；`serve_unix_lines` 可用 `socat - UNIX-CONNECT:/path/to/server.sock` 连接配置的 Unix socket，再发送同样的换行分隔 JSON。响应有 `id`，推送的 Topic 事件有 `method` 且没有 `id`。

## 订阅错误

WebSocket 和按行流对未知事件名返回 JSON-RPC 错误 `-32601`，对无效参数返回 `-32602`。HTTP SSE 对这两种情况返回 HTTP 400。订阅只接收建立后的新事件；重新连接会建立新的订阅。
