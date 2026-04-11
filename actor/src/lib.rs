//! # serviceless
//!
//! A small **async actor** library: each [`Service`] runs on a single mailbox, handlers are
//! `async`, and callers interact through [`ServiceAddress`] (typed messages) and optional
//! **topic** pub/sub ([`Topic`], [`TopicEndpoint`]). See the [`docs`] module for how to use the
//! actor model, runtime expectations, and topic semantics.
//!
//! ## Features
//!
//! - **Async actors** — Each [`Service`] runs a mailbox loop; hooks and [`Handler`] methods are
//!   `async` (via [`async_trait::async_trait`]).
//! - **Typed messages** — Implement [`Message`] and [`Handler`] for compile-time dispatch instead
//!   of manual routing for your own message types.
//! - **`call` and `send`** — [`ServiceAddress::call`] awaits `M::Result`; [`ServiceAddress::send`]
//!   enqueues work and drops the handler return value.
//! - **Topics (pub/sub-style)** — [`Topic`], [`RoutedTopic`], and [`TopicEndpoint`] for one-shot
//!   subscribe / publish flows still serialized through the actor mailbox.
//! - **External envelope streams** — [`Context::with_stream`] merges another [`futures_core::Stream`]
//!   of [`Envelope`]s with the internal mailbox on the same single-consumer path.
//! - **Typed narrowing** — [`ServiceAddress::into_address`] builds an [`Address`] for one message
//!   type plus a forwarding future you spawn alongside the main `run` future.
//! - **Bring your own runtime** — The crate returns a `run` future; you spawn it on Tokio or any
//!   other executor. There are **no optional Cargo `[features]`** on this crate today: the API
//!   surface is always enabled.
//!
//! ## Quick start
//!
//! 1. Implement [`Service`] for your actor state (pick [`EmptyStream`] if you
//!    do not merge an external envelope stream).
//! 2. Implement [`Message`] + [`Handler`] for request/reply or fire-and-forget work.
//! 3. Build a [`Context`], call [`Service::start_by_context`], then **spawn** the returned
//!    future on your async runtime.
//! 4. Use the returned [`ServiceAddress`] for [`ServiceAddress::call`], [`ServiceAddress::send`],
//!    and optionally [`ServiceAddress::subscribe`] for topics.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Greeter;
//!
//! #[async_trait]
//! impl Service for Greeter {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Hello(pub String);
//! impl Message for Hello {
//!     type Result = String;
//! }
//!
//! #[async_trait]
//! impl Handler<Hello> for Greeter {
//!     async fn handle(
//!         &mut self,
//!         msg: Hello,
//!         _ctx: &mut Context<Self, Self::Stream>,
//!     ) -> String {
//!         format!("Hello, {}", msg.0)
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (addr, run) = Greeter::default().start_by_context(Context::new());
//!     tokio::spawn(run);
//!     let reply = addr.call(Hello("Ada".into())).await.expect("reply");
//!     assert_eq!(reply, "Hello, Ada");
//!     addr.close_service();
//! }
//! ```
//!
//! Longer explanations, caveats, and pub/sub details live under [`docs`].

mod service;
pub use service::*;

mod context;
pub use context::*;

mod handler;
pub use handler::*;

mod topic;
pub use topic::*;

mod envelop;
pub use envelop::*;

mod error;
pub use error::*;

mod address;
pub use address::*;

mod service_address;
pub use service_address::*;

pub mod docs;
