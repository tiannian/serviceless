//! # Topics (pub/sub-style)
//!
//! Topics are **not** a separate network bus: they are still **serialized through the actor
//! mailbox**, just like ordinary messages. That keeps topic state and handler state consistent,
//! but it also means ordering and latency follow the same queue rules.
//!
//! ## Building a topic
//!
//! 1. Pick a marker type and implement [`crate::Topic`] with `type Item = …` for the payload
//!    broadcast to subscribers. The `Item` type must implement [`Clone`] because each waiting subscriber gets
//!    its own copy on [`crate::TopicEndpoint::publish`].
//! 2. Store a [`crate::TopicEndpoint`] field on your [`crate::Service`] (often `Default`-initialized).
//! 3. Implement [`crate::RoutedTopic`] for your marker type so `endpoint(service)` returns
//!    `&mut` that field. Keep routing **stable**: the same topic type must always resolve to the
//!    same field for a given service implementation.
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use serviceless::{
//!     Context, EmptyStream, Handler, Message, RoutedTopic, Service, Topic, TopicEndpoint,
//! };
//!
//! #[derive(Clone)]
//! pub struct UserReady(pub String);
//!
//! #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
//! pub struct UserReadyTopic(pub String);
//!
//! impl Topic for UserReadyTopic {
//!     type Item = UserReady;
//! }
//!
//! #[derive(Default)]
//! pub struct MyService {
//!     pub user_ready: TopicEndpoint<UserReadyTopic>,
//! }
//!
//! #[async_trait]
//! impl Service for MyService {
//!     type Stream = EmptyStream<Self>;
//! }
//!
//! impl RoutedTopic<MyService> for UserReadyTopic {
//!     fn endpoint(service: &mut MyService) -> &mut TopicEndpoint<Self> {
//!         &mut service.user_ready
//!     }
//! }
//!
//! pub struct DoWork;
//!
//! impl Message for DoWork {
//!     type Result = ();
//! }
//!
//! #[async_trait]
//! impl Handler<DoWork> for MyService {
//!     async fn handle(&mut self, _message: DoWork, ctx: &mut Context<Self, Self::Stream>) {
//!         let _ = ctx.publish_handle().publish::<UserReadyTopic>(UserReadyTopic("user-42".into()), UserReady("done".into()));
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let service = MyService::default();
//!     let (addr, run) = service.start_by_context(Context::new());
//!     tokio::spawn(run);
//!
//!     // Same enqueue/await pattern as `examples/topic.rs` (ordering vs. publish is discussed above).
//!     let ready = addr
//!         .subscribe::<UserReadyTopic>(UserReadyTopic("user-42".into()))
//!         .expect("subscribe should enqueue");
//!     addr.send(DoWork).expect("send work");
//!     let event = ready.await.expect("event");
//!     assert_eq!(event.0, "done");
//! }
//! ```
//!
//! ## Publishing
//!
//! - **Inside the actor** (common): [`crate::Context::publish`] enqueues a publish envelope. When
//!   it is processed, [`crate::TopicEndpoint::publish`] runs and wakes every waiter registered for
//!   that topic value at that moment, then **clears that value's waiter list**.
//! - Publishing is therefore a **one-shot fan-out**: each `subscribe` waits for **one** future
//!   publish on a specific topic value; after a publish, subscribers must `subscribe` again to
//!   receive another item for that value.
//!
//! ## Subscribing
//!
//! - **From outside:** [`crate::ServiceAddress::subscribe`] takes the **topic key** (same value you
//!   pass to [`crate::Context::publish`]), enqueues a subscribe envelope, and returns
//!   `Result<impl Future<Output = Result<Item>>>`. The outer [`crate::Result`] is `Err` when the
//!   mailbox is already closed; the inner future resolves to the next published `Item` for that key,
//!   or [`crate::Error::ServiceStoped`] if the actor stops before delivery.
//!
//! ## Caveats and patterns
//!
//! - **Ordering:** `subscribe` and `publish` both pass through the mailbox. If you `send` a
//!   message that publishes and **also** `subscribe` from another task without ordering guarantees,
//!   you can race: the publish envelope might be processed before the subscribe envelope. When
//!   you need a strict “subscribe then publish” handshake, **sequence** those operations (e.g.
//!   call `subscribe` and await its returned future before relying on a concurrent publish, or perform
//!   both from the same actor/handler).
//! - **Missed events:** there is **no** buffer of past publications for late subscribers—only
//!   waiters registered at publish time receive the item.
//! - **Cleanup:** [`crate::TopicEndpoint::clear`] drops pending waiters (their receivers observe a
//!   closed channel). Dropped receivers do not block publish; they are skipped.
//! - **Stopping:** when the service stops, pending topic operations fail like other mailbox
//!   operations.
//!
//! The repository’s `examples/topic.rs` shows the same wiring with assertions in a small demo.
