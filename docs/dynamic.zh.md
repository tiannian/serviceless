# 动态库 Service 设计

本文整理动态加载 Service 的接口与跨库协议。目标是把已有的 `Service` 和 `Handler<M>` 编译进原生动态库，由宿主按名称加载，并继续使用有类型的消息调用。当前 `dynamic` crate 是验证协议的原型；`export!` 宏尚未实现。

## 使用方式

期望的使用形式：

```rust,ignore
let loader = SrvDynLoader::new();
let s1 = loader.load_lib("S1", "path/to/library")?;

let result = s1.call(M1 { /* ... */ }).await?;
s1.send(M2 { /* ... */ })?;
s1.close().await;
```

`SrvDynLoader` 使用 `libloading` 打开库，查找服务的初始化和销毁入口。`call` 查找消息对应的调用入口，通过回调取得结果，再以 Future 的形式返回给调用方。`send` 只提交消息，不等待处理结果。加载器持有库句柄和实例指针，直到服务关闭。

加载器接受完整的库文件路径，也可以按平台的动态库命名规则从目录中生成文件名。实际的库文件名不必等于 Service 名称；例如 `flatten-so` 示例的库文件中导出名为 `S1` 的 Service。

## Service 导出

计划由宏生成跨库入口：

```rust,ignore
serviceless::export!(
    S1,
    name = "S1",
    messages = [M1, M2]
);
```

宏会为 `S1` 生成 `init`、`destroy`，以及每种消息的 `send` 和 `call` 入口。入口使用服务名和消息标识构造符号名；宏必须避免同名消息类型造成符号冲突。当前示例手写了 `__serviceless_S1_init`、`__serviceless_S1_destroy`、`__serviceless_S1_send_Add` 和 `__serviceless_S1_call_Add`，供后续实现宏时对照。

每个 `init` 调用创建一个独立实例，并返回不透明实例指针。后续入口只通过该指针访问实例，因此同一库可以创建多个 Service 实例。`destroy` 停止并释放对应实例。

导出函数采用 C ABI。消息和结果由使用者保证在宿主与动态库两侧定义一致、具有 C 兼容布局。第一版还要求它们是 `Copy`，且不包含需要跨库释放的资源。`repr(C)` 本身不能使 `String`、`Vec` 等 Rust 所有权类型适合直接跨库传递。当前原型以 `unsafe trait DynamicMessage` 表达这项由使用者承担的约束。

## 消息入口与回调

`send_M` 接收实例指针和消息值，将消息提交到 Service 邮箱。返回 `0` 表示提交成功；非零值表示错误。`call_M` 使用现有的 `ServiceAddress::call_raw` 取得结果接收端，然后异步等待处理结果：

```rust,ignore
type CallbackSuccess<R> = unsafe extern "C" fn(ctx: *mut c_void, result: *const R);
type CallbackFailed = unsafe extern "C" fn(ctx: *mut c_void, code: i32);

unsafe extern "C" fn call_M(
    instance: *mut c_void,
    message: M,
    ctx: *mut c_void,
    success: CallbackSuccess<M::Result>,
    failed: CallbackFailed,
) -> i32;
```

这里的 `ctx` 是加载器传入的回调上下文，与 Service 实例指针分别管理。`call_M` 返回 `0` 表示已接受调用：动态库取得回调上下文的所有权，之后必须恰好调用一次 `success` 或 `failed`。立即返回非零值表示调用未被接受，此时不得触发回调，加载器负责回收上下文。

`success` 中的结果指针仅在回调执行期间有效。加载器在回调内复制结果，通过一次性通道完成 `s1.call(...).await`；调用方不能保存该指针。异步调用 Future 被取消时，已被动态库接受的回调仍需执行，以便回收上下文。

## 运行时

初始化配置区分两种模式：

| 模式 | 目标行为 | 当前原型 |
| --- | --- | --- |
| 独立运行时 | 动态库创建自己的 Tokio 运行时和线程，运行 Service。 | 已实现，`flatten-so` 示例使用此模式。 |
| 共享运行时 | Service 使用加载器所在的 Tokio 运行时。 | 尚未实现。 |

最初的设想是在导出函数中调用 `TokioHandle::current()`，由加载器保证从 Tokio 运行时内调用它。实际验证发现，宿主与 `cdylib` 分别链接 Tokio 时，动态库中的线程局部运行时上下文与宿主不共享；即使加载器运行在 Tokio 任务中，动态库里的 `Handle::current()` 仍可能失败。因此共享模式需要补充跨库调度协议，当前示例会拒绝该模式。独立模式在库内持有运行时句柄，用它启动 Service 和等待 `call_raw` 结果的任务。

## 关闭与库生命周期

`close().await` 开始后拒绝新请求，等待所有已接受的调用完成回调，再销毁实例并卸载动态库。动态库必须保持加载，直到它的导出函数和回调任务都已退出；仅看到成功回调被调用还不足以判断动态库内的任务已经从回调返回。`flatten-so` 示例在销毁时等待这些任务退出。

第一版不提供强制关闭。如果 Handler 一直不返回，`close()` 也会一直等待。调用方应显式执行 `close()`；当前原型在仍有回调时直接丢弃加载结果，会保留库句柄，避免回调执行期间卸载代码。

## 当前示例

手写导出代码位于 [`dynamic/examples/flatten-so`](../dynamic/examples/flatten-so/src/lib.rs)，加载示例位于 [`dynamic/examples/load_flatten.rs`](../dynamic/examples/load_flatten.rs)。从仓库根目录运行：

```sh
cargo build -p serviceless-flatten-so
cargo run -p serviceless-dynamic --example load_flatten -- target/debug
```

加载示例使用 `libloading::library_filename` 选择当前平台的动态库文件名，依次执行 `call`、`send` 和 `close`。后续实现 `export!` 时，应使宏生成的入口与这套加载协议一致。
