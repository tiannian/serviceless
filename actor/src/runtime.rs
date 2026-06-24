use std::future::Future;

use async_trait::async_trait;

pub trait Runtime: Send {
    type Error;

    type UnboundedSender<T>: UnboundedSender<T, Error = Self::Error>;

    type UnboundedReceiver<T>: UnboundedReceiver<T, Error = Self::Error>;

    fn unbounded<T>(capacity: usize) -> (Self::UnboundedSender<T>, Self::UnboundedReceiver<T>);

    type Spawner<T>: Spawner<T, Error = Self::Error>;

    fn spawner<T>() -> Self::Spawner<T>;
}

pub trait UnboundedSender<T> {
    type Error;

    fn send(&self, item: T) -> Result<(), Self::Error>;

    fn is_closed(&self) -> bool;
}

pub trait UnboundedReceiver<T> {
    type Error;

    fn recv(&mut self) -> Result<T, Self::Error>;

    fn close(&mut self);
}

#[async_trait]
pub trait Spawner<T> {
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
