use std::error::Error;

use async_trait::async_trait;

use crate::{Handler, Message, Service};

#[async_trait]
pub trait ServiceAddress<S>: Clone
where
    S: Service,
{
    type Error: Error;

    async fn is_stop(&self) -> bool;

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
