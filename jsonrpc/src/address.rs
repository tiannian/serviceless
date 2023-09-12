use serviceless_core::{Address, Handler, Message, Service};

use crate::{Error, Result, RpcClient, RpcResponse};

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

impl JsonRpcAddress {
    pub async fn is_stop(&self) -> bool {
        false
    }

    /*     async fn call<M>(&self, message: M) -> Result<M::Result>
    where
        M: Message + Send + 'static,
        S: Handler<M>,
        M::Result: Send,
    {
        let r: RpcResponse<M::Result> = self.client.call(message).await?;
    } */

    pub fn send<M>(&self, message: M) -> Result<()>
    where
        M: Message + Send + 'static,
        M::Result: Send,
    {
        Ok(())
    }
}
