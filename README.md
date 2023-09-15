# Serviceless

Serviceless is an simple actor model in rust, like actix but simple and async.

Currently, this crate only use tokio as backend.

This crate is no unsafe code.

## Usage

This crate provide API same like `actix`. But all api is async include handler.

### Service

`Service` is a `Actor` in Actor Model.

We can impl an `Service` on an struct to delcare an `Service`.

```rust
pub struct Service0 {}

impl Service for Service0 {}
```

The `Service` provide hook named `started` and `stopped`, them all are async.
Please use `async_trait` macros.

```rust
pub struct Service1 {}

#[async_trait]
impl Service for Service1 {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Started")
    }
}
```

#### Start Service

Start a service is very simple, only create it and call `start` method.

```rust
let svc = Service1 {};
svc.start();
```

> Note: this function must call in async function or after async runtime initialized.
> If not, it will panic.

#### Stop and Pause

When a service started, we can call stop and pause method on `context`.

You can call these function in Service Hook or in Handler.

### Handler and Mesaage

A service can sending an message to other service, or called by other service.

#### Message

To call other Service, we must declare an `Message` first.

Any type can be a message, only need impl Message trait on struct
and define an result.

```rust
pub struct Messagep {}

pub struct MessageResult {}

impl Message for Message0 {
    type Result = MessageResult;
}
```

#### Handler

Impl Handler on service, we can make a service accept call from other service.

```rust
#[async_trait]
impl Handler<U8> for Service0 {
    async fn handler(&mut self, message: U8, _ctx: &mut Context<Self>) -> U8 {
        U8(message.0 + 2)
    }
}
```

Handler also an async function, please use `async_trait` macros.

### Address

When we start an service, we can get an address. We also can get it from Context.

#### Call and Send

The address can make call or send.

- Call means caller want to known the result.
  1. This is an async function
  2. When an service stop, caller will get `ServiceStopped` from Error.
  3. When an service pause, caller will get `ServicePaused` from Error.
- Send means caller don't care the result, so
  1. This is an plain function
  2. When an service stop, caller will get `ServiceStopped` from Error.
  3. Caller can't known service paused
