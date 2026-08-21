use crate::runtime::Runtime;
use async_trait::async_trait;
use tokio::task::JoinSet;

use super::{
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
    type AsyncUnboundedReceiver<T>
        = TokioUnboundedReceiver<T>
    where
        T: Send;

    fn async_unbounded<T>() -> (Self::UnboundedSender<T>, Self::AsyncUnboundedReceiver<T>)
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
