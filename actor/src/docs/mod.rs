//! # User guide
//!
//! Practical notes for **using** this crate: how the actor model behaves in code, what to watch
//! for at runtime, and how **topics** (pub/sub-style notifications) differ from ordinary
//! messages. API details stay on the individual types; this module is the narrative layer.
//!
//! ## Features
//!
//! The crate root documents the full **feature list** (async services, typed messaging,
//! `call`/`send`, topics, merged streams, `into_address`, and no optional Cargo feature flags).
//! Start there for a one-screen summary, then use the sections below for usage and caveats.
//!
//! ## Sections
//!
//! | Module | What you will find |
//! |--------|--------------------|
//! | [`overview`] | Mental model: mailbox, single-threaded actor, addresses |
//! | [`services`] | Implementing [`crate::Service`], starting and stopping |
//! | [`messaging`] | [`crate::Message`], [`crate::Handler`], [`crate::ServiceAddress::call`] vs [`crate::ServiceAddress::send`] |
//! | [`pubsub`] | [`crate::Topic`], [`crate::RoutedTopic`], [`crate::TopicEndpoint`], subscribe/publish patterns |
//! | [`runtime`] | Spawning the run future, extra streams, ordering and lost messages |
//!
//! ## Minimal program
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{Context, EmptyStream, Service};
//!
//! #[derive(Default)]
//! struct MyActor;
//!
//! #[async_trait]
//! impl Service for MyActor {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let actor = MyActor::default();
//!     let ctx = Context::new();
//!     let (addr, run) = actor.start_by_context(ctx);
//!     tokio::spawn(run);
//!     let _ = addr; // clone and use for `call` / `send` / `subscribe`
//! }
//! ```

pub mod messaging;
pub mod overview;
pub mod pubsub;
pub mod runtime;
pub mod services;
