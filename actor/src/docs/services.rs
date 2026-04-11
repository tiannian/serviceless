//! # Services: implementation and lifecycle
//!
//! ## Implementing [`crate::Service`]
//!
//! - Define your actor `struct` and implement [`crate::Service`]. Choose `type Stream = …`
//!   matching how you construct [`crate::Context`]: use [`crate::EmptyStream`] with
//!   [`crate::Context::new`], or a custom stream with [`crate::Context::with_stream`].
//! - Use [`async_trait::async_trait`] on the impl if you override async hooks.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Service};
//!
//! #[derive(Default)]
//! struct Worker;
//!
//! #[async_trait]
//! impl Service for Worker {
//!     type Stream = EmptyStream<Self>;
//!
//!     async fn started(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
//!         // runs once before the mailbox loop
//!     }
//!
//!     async fn stopped(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
//!         // runs after the mailbox closes and pending envelopes are drained
//!     }
//! }
//! ```
//!
//! ## Starting a service
//!
//! Typical pattern:
//!
//! 1. `let ctx = Context::new();` (or `Context::with_stream(your_stream)`).
//! 2. `let (addr, run) = my_service.start_by_context(ctx);` (equivalently [`crate::Context::run`]).
//! 3. Spawn `run` on your runtime, e.g. `tokio::spawn(run);`.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Service};
//!
//! #[derive(Default)]
//! struct Worker;
//!
//! #[async_trait]
//! impl Service for Worker {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let worker = Worker::default();
//!     let ctx = Context::new();
//!     let (addr, run) = worker.start_by_context(ctx);
//!     tokio::spawn(run);
//!     // `addr` is usable immediately, but work is only processed after `run` is polled.
//!     let _ = addr;
//! }
//! ```
//!
//! Until the run future is polled, **no** mailbox processing happens; queued work from `addr`
//! will not execute.
//!
//! ## Stopping
//!
//! - From inside the actor: [`crate::Context::stop`] closes the mailbox sender; when the
//!   channel drains, the run loop ends and [`crate::Service::stopped`] runs.
//! - From outside: [`crate::ServiceAddress::close_service`] has the same effect.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Handler, Message, Service};
//!
//! #[derive(Default)]
//! struct Worker;
//!
//! #[async_trait]
//! impl Service for Worker {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! struct Stop;
//! impl Message for Stop {
//!     type Result = ();
//! }
//!
//! #[async_trait]
//! impl Handler<Stop> for Worker {
//!     async fn handle(
//!         &mut self,
//!         _: Stop,
//!         ctx: &mut Context<Self, Self::Stream>,
//!     ) {
//!         ctx.stop();
//!     }
//! }
//! ```
//!
//! ## Caveats
//!
//! - **[`crate::Context::addr`] before start** is allowed, but if you send before the run future
//!   is spawned and advancing, **messages can be lost** or the service may not yet be listening.
//! - **Runtime:** starting or driving the mailbox **outside** a live async runtime is unsupported
//!   and can panic; see [`crate::docs::runtime`].
