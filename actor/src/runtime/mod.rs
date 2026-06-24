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

    type UnboundedSender<T>: UnboundedSender<T, Error = Self::Error>;
    type UnboundedReceiver<T>: UnboundedReceiver<T, Error = Self::Error>;

    fn unbounded<T>() -> (Self::UnboundedSender<T>, Self::UnboundedReceiver<T>);

    type OneshotSender<T>: OneshotSender<T, Error = Self::Error>;
    type OneshotReceiver<T>: OneshotReceiver<T, Error = Self::Error>;

    fn oneshot<T>() -> (Self::OneshotSender<T>, Self::OneshotReceiver<T>);

    type Spawner<T>: Spawner<T, Error = Self::Error>;

    fn spawner<T>() -> Self::Spawner<T>;
}
