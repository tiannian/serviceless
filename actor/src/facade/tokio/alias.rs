use crate::{
    runtime_impl::tokio::TokioRuntime, RuntimedReplyHandle, RuntimedTopicAllHandle,
    RuntimedTopicEndpoint,
};

pub type ReplyHandle<M> = RuntimedReplyHandle<M, TokioRuntime>;
pub type TopicAllHandle<T> = RuntimedTopicAllHandle<T, TokioRuntime>;
pub type TopicEndpoint<T> = RuntimedTopicEndpoint<T, TokioRuntime>;
