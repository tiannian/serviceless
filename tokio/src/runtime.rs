use async_trait::async_trait;
use serviceless_actor::Runtime;
use tokio::task::JoinSet;

use crate::{
    Error, TokioOneshotReceiver, TokioOneshotSender, TokioSpawner, TokioUnboundedReceiver,
    TokioUnboundedSender,
};

pub struct TokioRuntime;

#[async_trait]
impl Runtime for TokioRuntime {
    type Error = Error;

    type UnboundedSender<T>
        = TokioUnboundedSender<T>
    where
        T: Send;
    type UnboundedReceiver<T>
        = TokioUnboundedReceiver<T>
    where
        T: Send;

    fn unbounded<T>() -> (Self::UnboundedSender<T>, Self::UnboundedReceiver<T>)
    where
        T: Send,
    {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (
            TokioUnboundedSender { sender },
            TokioUnboundedReceiver { receiver },
        )
    }

    type OneshotSender<T>
        = TokioOneshotSender<T>
    where
        T: Send;
    type OneshotReceiver<T>
        = TokioOneshotReceiver<T>
    where
        T: Send;

    fn oneshot<T>() -> (Self::OneshotSender<T>, Self::OneshotReceiver<T>)
    where
        T: Send,
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        (
            TokioOneshotSender { sender },
            TokioOneshotReceiver { receiver },
        )
    }

    type Spawner<T>
        = TokioSpawner<T>
    where
        T: Send + 'static;

    fn spawner<T>() -> Self::Spawner<T>
    where
        T: Send + 'static,
    {
        TokioSpawner {
            tasks: JoinSet::new(),
        }
    }
}
