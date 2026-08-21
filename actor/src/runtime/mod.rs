mod unbound;
pub use unbound::*;

mod oneshot;
pub use oneshot::*;

mod spawn;
pub use spawn::*;

mod inner;
pub use inner::*;

pub trait Runtime: Send + 'static {
    type Error: Send;

    type AsyncUnboundedSender<T>: UnboundedSender<T, Error = Self::Error>
    where
        T: Send;
    type AsyncUnboundedReceiver<T>: AsyncUnboundedReceiver<T, Error = Self::Error>
    where
        T: Send;

    type SyncUnboundedSender<T>: UnboundedSender<T, Error = Self::Error>
    where
        T: Send;

    type SyncUnboundedReceiver<T>: SyncUnboundedReceiver<T, Error = Self::Error>
    where
        T: Send;

    fn async_unbounded<T>() -> (
        Self::AsyncUnboundedSender<T>,
        Self::AsyncUnboundedReceiver<T>,
    )
    where
        T: Send;

    fn sync_unbounded<T>() -> (Self::SyncUnboundedSender<T>, Self::SyncUnboundedReceiver<T>)
    where
        T: Send;

    type OneshotSender<T>: OneshotSender<T, Error = Self::Error>
    where
        T: Send;
    type OneshotReceiver<T>: OneshotReceiver<T, Error = Self::Error>
    where
        T: Send;

    fn oneshot<T>() -> (Self::OneshotSender<T>, Self::OneshotReceiver<T>)
    where
        T: Send;

    type Spawner<T>: Spawner<T, Error = Self::Error>
    where
        T: Send + 'static;

    fn spawner<T>() -> Self::Spawner<T>
    where
        T: Send + 'static;
}
