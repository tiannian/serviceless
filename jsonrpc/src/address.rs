use async_trait::async_trait;
use serviceless_core::{Address, Handler, Message, Service};

use crate::{Error, Result, RpcClient};

#[derive(Clone)]
pub struct JsonRpcAddress {
    client: RpcClient,
}

impl JsonRpcAddress {
    pub fn new(url: &str, jwt: Option<&[u8]>) -> Result<Self> {
        let client = RpcClient::new(url, jwt)?;

        Ok(Self { client })
    }
}

#[async_trait]
impl<S> Address<S> for JsonRpcAddress
where
    S: Service,
{
    type Error = Error;

    async fn is_stop(&self) -> bool {
        false
    }

    async fn call<M>(&self, message: M) -> Result<M::Result>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
    }

    fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
        Ok(())
    }
}
