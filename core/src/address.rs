use std::error::Error;

use async_trait::async_trait;

use crate::{Handler, Message, Service};

/// Address of service
#[async_trait]
pub trait Address<S>: Clone
where
    S: Service,
{
    /// Error of address
    type Error: Error;

    /// Return true when service stopped.
    async fn is_stop(&self) -> bool;

    /// Call service's handler and get result
    async fn call<M>(&self, message: M) -> Result<M::Result, Self::Error>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send;

    /// Call service's handler without result
    ///
    /// Beacuse this function don't need result, so it can call without async.
    fn send<M>(&self, message: M) -> Result<(), Self::Error>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send;
}
