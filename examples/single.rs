use async_trait::async_trait;
use serviceless::{Context, Handler, Message, Service};

#[derive(Debug, Default)]
pub struct Service0 {}

#[async_trait]
impl Service for Service0 {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Started")
    }
}

#[derive(Debug)]
pub struct U8(pub u8);

impl Message for U8 {
    type Result = U8;
}

#[async_trait]
impl Handler<U8> for Service0 {
    async fn handler(&mut self, message: U8, _ctx: &mut Context<Self>) -> U8 {
        U8(message.0 + 2)
    }
}

#[tokio::main]
async fn main() {
    let addr = Service0::default().start();

    let res = addr.call(U8(8)).await.unwrap();

    println!("{:?}", res)
}
