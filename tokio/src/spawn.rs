use async_trait::async_trait;
use serviceless_actor::{Spawner, TaskHandle};
use tokio::task::{AbortHandle, JoinSet};

use crate::Error;

pub struct TokioSpawner<T> {
    pub(crate) tasks: JoinSet<T>,
}

#[async_trait]
impl<T> Spawner<T> for TokioSpawner<T>
where
    T: Send + 'static,
{
    type Error = Error;

    type TaskHandle = TokioTaskHandle;

    fn len(&self) -> usize {
        self.tasks.len()
    }

    fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn spawn<F>(&mut self, task: F) -> Self::TaskHandle
    where
        F: Future<Output = T> + Send + 'static,
        T: Send,
    {
        let handle = self.tasks.spawn(task);
        TokioTaskHandle { handle }
    }

    fn spawn_blocking<F>(&mut self, f: F) -> Self::TaskHandle
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send,
    {
        let handle = self.tasks.spawn_blocking(f);
        TokioTaskHandle { handle }
    }

    async fn join_next(&mut self) -> Option<Result<T, Self::Error>> {
        if let Some(result) = self.tasks.join_next().await {
            let res = result.map_err(Error::JoinError);

            Some(res)
        } else {
            None
        }
    }
}

pub struct TokioTaskHandle {
    pub(crate) handle: AbortHandle,
}

impl TaskHandle for TokioTaskHandle {
    fn abort(&self) {
        self.handle.abort();
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}
