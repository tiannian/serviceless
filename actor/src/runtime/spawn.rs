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
}

pub trait TaskHandle {
    fn abort(&self);

    fn is_finished(&self) -> bool;
}
