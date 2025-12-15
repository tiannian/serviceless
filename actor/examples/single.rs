use async_trait::async_trait;
use serviceless_actor::{Context, Handler, Message, Service};

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
    async fn handle(&mut self, message: U8, _ctx: &mut Context<Self>) -> U8 {
        U8(message.0 + 2)
    }
}

#[derive(Debug)]
pub struct U16(pub u16);

impl Message for U16 {
    type Result = U16;
}

#[async_trait]
impl Handler<U16> for Service0 {
    async fn handle(&mut self, message: U16, _ctx: &mut Context<Self>) -> U16 {
        U16(message.0 + 300)
    }
}

#[tokio::main]
async fn main() {
    let srv = Service0::default();

    let (addr, future) = srv.start();
    tokio::spawn(future);

    let res = addr.call(U8(8)).await.unwrap();

    println!("{:?}", res);

    let res = addr.call(U16(8)).await.unwrap();

    println!("{:?}", res);
}
