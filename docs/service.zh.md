# Service 与消息：使用宏定义方法

[English](service.md)

Service 通过一个邮箱处理有类型的消息。一个可调用的“方法”由消息类型和该 Service 对应的 `Handler` 实现共同定义。

## 定义 Service

将 `#[service]` 标在 `impl Service for 类型` 上：

```rust
use serviceless::{service, Service};

struct Counter(u32);

#[service(name = "counter")]
impl Service for Counter {}
```

宏会补上 `type Runtime = TokioRuntime` 和 `type Stream = EmptyStream<Self>`。没有指定 `name` 时，元数据名称默认是类型名。如需更换运行时，可传入字符串形式的 Rust 类型路径，例如 `#[service(runtime = "path::CustomRuntime")]`。显式定义的 `type Stream` 或 `metadata()` 会被保留。使用宏时，不能同时指定 `name` 和实现 `metadata()`，也不能在 impl 中重复定义 `Runtime`。

宏还会给这个 impl 添加 `async_trait`，因此可以直接定义生命周期方法：

```rust,ignore
#[service]
impl Service for Counter {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        // 开始处理邮箱前调用。
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        // 处理循环退出时调用。
    }
}
```

## 定义消息与 Handler

`#[derive(Message)]` 配合 `#[message(result = 类型)]` 生成 `Message::Result`。消息可以是结构体或枚举，也支持泛型：

```rust
use serviceless::Message;

#[derive(Message)]
#[message(result = u32)]
struct Add(u32);
```

实现 `Handler<Add>` 来定义 Service 如何处理这条消息。Handler 的实现仍需 `#[async_trait]`：

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

一个 Service 可以分别实现多个 `Handler<M>`。需要延后回复时，可以覆盖 `handle_preferred`，通过 `ReplyHandle<M>` 发送结果。通常只需实现 `handle`；默认的 `handle_preferred` 会调用它并回复。

## 运行并调用 Service

`Context::new(&service).run(service)` 返回地址和 Future，需要用异步运行时启动这个 Future：

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

`call(...).await` 会等待结果；`send(...)` 只将消息放入邮箱。Service 停止后投递可能失败；消息及结果还需满足调用 API 的 `Send` 约束。如需接入额外的 `Envelope` 流，可显式声明 `type Stream` 并使用 `Context::with_stream`。

完整示例见 [`serviceless/examples/service_macro.rs`](../serviceless/examples/service_macro.rs)。Topic 的订阅和发布见 [Topic 文档](topic.zh.md)。
