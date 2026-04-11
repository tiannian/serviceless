//! # Runtime, streams, and operational notes
//!
//! ## Async runtime
//!
//! The crate is built around **futures** and async handlers. You must drive the **`run` future**
//! returned from [`crate::Service::start_by_context`] / [`crate::Context::run`] on an async
//! executor (for example Tokio). Constructing contexts or addresses without a running executor
//! where channel internals expect one can **panic**—treat “after runtime init” as a hard
//! requirement.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Service};
//!
//! #[derive(Default)]
//! struct Actor;
//!
//! #[async_trait]
//! impl Service for Actor {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (addr, run) = Actor::default().start_by_context(Context::new());
//!     tokio::spawn(run);
//!     let _ = addr;
//! }
//! ```
//!
//! ## Merging extra input ([`crate::Context::with_stream`])
//!
//! If `type Stream` is not the empty stream, construct [`crate::Context::with_stream`] with the
//! same stream type your [`crate::Service`] declares. Envelopes from [`crate::ServiceAddress`] and
//! from your stream are **merged**; the actor still handles **one** envelope at a time, but
//! ordering between the two sources is interleaved as the `select` yields them.
//!
//! Use [`crate::Context::stream`] to access the user half of that merged stream when you need to
//! poll or reconfigure it from inside the actor.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use futures_util::{future::ready, stream::once};
//! use serviceless::{Context, Envelope, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Gate;
//!
//! struct Knock(pub u8);
//! impl Message for Knock {
//!     type Result = u8;
//! }
//!
//! #[async_trait]
//! impl Handler<Knock> for Gate {
//!     async fn handle(&mut self, m: Knock, _ctx: &mut Context<Self, Self::Stream>) -> u8 {
//!         m.0
//!     }
//! }
//!
//! #[async_trait]
//! impl Service for Gate {
//!     type Stream =
//!         futures_util::stream::Once<futures_util::future::Ready<Envelope<Self>>>;
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let stream = once(ready(Envelope::new(Knock(7))));
//!     let ctx = Context::with_stream(stream);
//!     let (_addr, run) = Gate::default().start_by_context(ctx);
//!     tokio::spawn(run);
//! }
//! ```
//!
//! ## Lost or early traffic
//!
//! - Obtaining [`crate::Context::addr`] before the run future is spawned can expose a live sender
//!   while **no** task is processing the mailbox yet; sends may fail or appear lost depending on
//!   timing.
//! - After [`crate::Context::stop`] or channel close, new `call` / `send` / `subscribe` attempts
//!   surface as stopped errors—design supervisors or shutdown ordering accordingly.
