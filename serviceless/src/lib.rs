#![doc = include_str!("../../README.md")]

pub use serviceless_actor::RuntimedService as Service;
pub use serviceless_actor::*;
pub use serviceless_tokio::TokioRuntime;

pub type ReplyHandle<M> = RuntimedReplyHandle<M, TokioRuntime>;
pub type TopicAllHandle<T> = RuntimedTopicAllHandle<T, TokioRuntime>;
pub type TopicEndpoint<T> = RuntimedTopicEndpoint<T, TokioRuntime>;
