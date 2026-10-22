#![doc = include_str!("../README.md")]

extern crate self as serviceless;

pub use serviceless_actor::RuntimedService as Service;
pub use serviceless_actor::*;
pub use serviceless_macros::{service, Message, Topic, TopicRoutes};
pub use serviceless_tokio::TokioRuntime;

#[doc(hidden)]
pub mod __private {
    pub use async_trait::async_trait;
}

pub type ReplyHandle<M> = RuntimedReplyHandle<M, TokioRuntime>;
pub type TopicAllHandle<T> = RuntimedTopicAllHandle<T, TokioRuntime>;
pub type TopicEndpoint<T> = RuntimedTopicEndpoint<T, TokioRuntime>;
