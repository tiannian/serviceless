//! # Actor model overview
//!
//! ## What you are building
//!
//! A **service** is an actor: a `struct` that owns state and processes a **mailbox** of
//! [`crate::Envelope`] items one at a time. Concurrency comes from *many* services and async
//! I/O inside a handler—not from running one service’s handlers in parallel. That single-queue
//! discipline is what makes reasoning about mutable actor state straightforward.
//!
//! ## Main pieces
//!
//! - **[`crate::Service`]** — Actor type and lifecycle hooks (`started`, `stopped`).
//! - **[`crate::Context`]** — Mailbox handle from inside the actor; also used to build the
//!   [`crate::ServiceAddress`] and to publish topics.
//! - **[`crate::ServiceAddress`]** — Cheaply clonable handle for **other** tasks to `call`, `send`,
//!   or `subscribe` without touching actor internals.
//! - **[`crate::Handler`]** / **[`crate::Message`]** — Typed work units delivered through the mailbox.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service, ServiceAddress};
//!
//! #[derive(Default)]
//! struct Counter {
//!     n: u32,
//! }
//!
//! #[async_trait]
//! impl Service for Counter {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Inc;
//! impl Message for Inc {
//!     type Result = u32;
//! }
//!
//! #[async_trait]
//! impl Handler<Inc> for Counter {
//!     async fn handle(
//!         &mut self,
//!         _: Inc,
//!         _ctx: &mut Context<Self, Self::Stream>,
//!     ) -> u32 {
//!         self.n += 1;
//!         self.n
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (addr, run): (ServiceAddress<Counter>, _) =
//!         Counter::default().start_by_context(Context::new());
//!     tokio::spawn(run);
//!     let n = addr.call(Inc).await.expect("call");
//!     assert_eq!(n, 1);
//!     addr.close_service();
//! }
//! ```
//!
//! ## Things to keep in mind
//!
//! - **All mailbox work is ordered.** Messages, topic subscribe envelopes, and topic publish
//!   envelopes are processed in arrival order. Design protocols accordingly (see [`crate::docs::pubsub`]).
//! - **`call` waits for the actor.** Nested `call` chains between actors can deadlock if you are
//!   not careful; prefer `send` or structure actors so they do not wait on each other’s `call`
//!   in a cycle.
//! - **Stopping closes the mailbox.** After stop, sends return [`crate::Error::ServiceStoped`]
//!   (or the async equivalent for `call` / `subscribe`).
