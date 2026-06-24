mod unbound;
pub use unbound::*;

mod oneshot;
pub use oneshot::*;

mod spawn;
pub use spawn::*;

mod inner;
pub use inner::*;

pub trait Runtime: Send + 'static {
    type Error;

    type UnboundedSender<T>: UnboundedSender<T, Error = Self::Error>
    where
        T: Send;
    type UnboundedReceiver<T>: UnboundedReceiver<T, Error = Self::Error>
    where
        T: Send;

    fn unbounded<T>() -> (Self::UnboundedSender<T>, Self::UnboundedReceiver<T>)
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

    type Spawner<T>: Spawner<T, Error = Self::Error>;

    fn spawner<T>() -> Self::Spawner<T>;
}
