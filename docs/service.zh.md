# Service 与消息：使用宏定义方法

[English](service.md)

本文基于当前仓库的 `serviceless` facade API。一个 Service 通过邮箱顺序处理消息；对外可调用的“方法”由消息类型和该 Service 的 `Handler<消息类型>` 实现共同定义。

## 定义并运行 Service

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

`#[service]` 必须标在 `impl Service for 类型` 上。它自动补上 `type Runtime = TokioRuntime`、`type Stream = EmptyStream<Self>`，并以类型名生成 `metadata().name`。也可以用 `#[service(name = "counter", runtime = "path::CustomRuntime")]` 指定名称和运行时；`runtime` 的值是字符串形式的 Rust 类型路径。若自己实现 `type Stream` 或 `metadata()`，宏会保留它们。`name` 与显式的 `metadata()` 不能同时使用；`Runtime` 应通过宏参数指定，不能在 impl 中重复定义。

`#[service]` 会为 Service 实现添加 `async_trait`，所以 `started` 和 `stopped` 生命周期方法可以直接写在该 impl 中：

```rust,ignore
#[service]
impl Service for Counter {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        // Service 开始处理邮箱前调用。
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        // Service 退出处理循环时调用。
    }
}
```

## 定义消息和 Handler

`#[derive(Message)]` 配合 `#[message(result = 类型)]` 生成 `Message::Result`。消息可以是结构体或枚举；带泛型的消息也受支持。一个 Service 可以分别实现多个 `Handler<M>`，每个消息类型对应一个处理方法。

`Handler` 的实现仍需 `#[async_trait]`。`handle` 接收消息和当前 Service 的 `Context`，返回消息声明的结果类型。`address.call(message).await` 等待结果；`address.send(message)` 只将消息放入邮箱，不等待处理结果。两者的错误都可能表示 Service 已停止。消息及其结果需满足调用 API 的 `Send` 约束。

如果需要在处理过程中延后回复，可以覆盖 `Handler::handle_preferred` 并通过 `ReplyHandle<M>` 发送结果；普通场景只需实现 `handle`，默认实现会调用它并回复。

`Context::new(&service).run(service)` 返回地址和运行 Future，调用方需要用异步运行时启动该 Future。若要接入额外的 `Envelope` 流，可显式声明 `type Stream` 并使用 `Context::with_stream`。

完整示例见 [`serviceless/examples/service_macro.rs`](../serviceless/examples/service_macro.rs)。Topic 的定义和发布见 [Topic 文档](topic.zh.md)。
