use std::future::Future;

use async_trait::async_trait;

#[async_trait]
pub trait Spawner<T>: Send {
    type Error;

    type TaskHandle: TaskHandle;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn spawn<F>(&mut self, task: F) -> Self::TaskHandle
    where
        F: Future<Output = T> + Send + 'static,
        T: Send;

    fn spawn_blocking<F>(&mut self, f: F) -> Self::TaskHandle
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send;

    async fn join_next(&mut self) -> Option<Result<T, Self::Error>>;

    /// Non-blocking poll: returns the next completed task's result if one is
    /// already ready, otherwise returns `None` immediately without awaiting.
    ///
    /// Used by the actor run loop to batch-drain completed tasks before
    /// awaiting new events, preventing a busy loop when many tasks pile up.
    fn try_join_next(&mut self) -> Option<Result<T, Self::Error>>;
}

pub trait TaskHandle: Send + Sync + 'static {
    fn abort(&self);

    fn is_finished(&self) -> bool;
}
