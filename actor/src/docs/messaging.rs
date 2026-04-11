//! # Messages and addresses
//!
//! ## Defining work
//!
//! - Implement [`crate::Message`] for a payload type and set `type Result` to the value returned
//!   to callers of [`crate::ServiceAddress::call`] (use `()` if you do not need a reply).
//! - Implement [`crate::Handler`] for your [`crate::Service`] and each `M` you handle. Handlers are
//!   `async` and receive `&mut Context<Self, Self::Stream>` for publishing, stopping, or
//!   grabbing [`crate::Context::addr`].
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Echo;
//!
//! #[async_trait]
//! impl Service for Echo {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Line(pub String);
//! impl Message for Line {
//!     type Result = String;
//! }
//!
//! #[async_trait]
//! impl Handler<Line> for Echo {
//!     async fn handle(
//!         &mut self,
//!         msg: Line,
//!         _ctx: &mut Context<Self, Self::Stream>,
//!     ) -> String {
//!         msg.0
//!     }
//! }
//! ```
//!
//! ## [`crate::ServiceAddress::call`] vs [`crate::ServiceAddress::send`]
//!
//! - **`call`** — Enqueues a message **with** a one-shot reply channel; awaits `M::Result`.
//!   If the service stops before the reply is sent, you get [`crate::Error::ServiceStoped`].
//!   `M::Result` must be `Send` because the reply crosses tasks.
//! - **`send`** — Enqueues the same handler path but **drops** the result. Synchronous for the
//!   caller; still subject to mailbox ordering and backpressure characteristics of the
//!   unbounded channel.
//!
//! Use `send` when no return value is needed; use `call` when the caller must observe completion
//! or a computed value.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Echo;
//!
//! #[async_trait]
//! impl Service for Echo {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Line(pub String);
//! impl Message for Line {
//!     type Result = String;
//! }
//!
//! #[async_trait]
//! impl Handler<Line> for Echo {
//!     async fn handle(
//!         &mut self,
//!         msg: Line,
//!         _ctx: &mut Context<Self, Self::Stream>,
//!     ) -> String {
//!         msg.0
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (addr, run) = Echo::default().start_by_context(Context::new());
//!     tokio::spawn(run);
//!
//!     let out = addr.call(Line("hi".into())).await.expect("reply");
//!     assert_eq!(out, "hi");
//!
//!     addr.send(Line("fire-and-forget".into())).expect("send");
//!     addr.close_service();
//! }
//! ```
//!
//! ## Caveats
//!
//! - **Handler panics** will tear down the task running the actor; treat handler bodies like
//!   other async task entry points (avoid `unwrap` on external inputs where possible).
//! - **Backpressure:** the mailbox is **unbounded**. A fast producer can grow memory without
//!   blocking; throttle at the edges if producers can outrun the actor.
//! - **Typed forwarding:** [`crate::ServiceAddress::into_address`] builds a narrow
//!   [`crate::Address`] plus a forwarding future you must **spawn** like the main run future.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Echo;
//!
//! #[async_trait]
//! impl Service for Echo {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Line(pub String);
//! impl Message for Line {
//!     type Result = String;
//! }
//!
//! #[async_trait]
//! impl Handler<Line> for Echo {
//!     async fn handle(
//!         &mut self,
//!         msg: Line,
//!         _ctx: &mut Context<Self, Self::Stream>,
//!     ) -> String {
//!         msg.0
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (addr, run) = Echo::default().start_by_context(Context::new());
//!     tokio::spawn(run);
//!
//!     let (line_addr, forward) = addr.clone().into_address::<Line>();
//!     tokio::spawn(forward);
//!
//!     let out = line_addr.call(Line("narrow".into())).await.expect("reply");
//!     assert_eq!(out, "narrow");
//!     addr.close_service();
//! }
//! ```
