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
    async fn handler(&mut self, message: U8, _ctx: &mut Context<Self>) -> U8 {
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
    async fn handler(&mut self, message: U16, _ctx: &mut Context<Self>) -> U16 {
        U16(message.0 + 300)
    }
}

#[tokio::main]
async fn main() {
    let srv = Service0::default();

    let (service_addr, future) = srv.start();
    tokio::spawn(future);

    // Test ServiceAddress with multiple message types
    println!("=== Testing ServiceAddress ===");
    let res = service_addr.call(U8(8)).await.unwrap();
    println!("ServiceAddress call U8(8): {:?}", res);

    let res = service_addr.call(U16(8)).await.unwrap();
    println!("ServiceAddress call U16(8): {:?}", res);

    // Test Address<M> for specific message types
    println!("\n=== Testing Address<U8> ===");
    let (addr_u8, forward_future_u8) = service_addr.clone().into_address::<U8>();
    tokio::spawn(forward_future_u8);

    let res = addr_u8.call(U8(10)).await.unwrap();
    println!("Address<U8> call U8(10): {:?}", res);

    addr_u8.send(U8(20)).unwrap();
    println!("Address<U8> send U8(20): success");

    println!("\n=== Testing Address<U16> ===");
    let (addr_u16, forward_future_u16) = service_addr.clone().into_address::<U16>();
    tokio::spawn(forward_future_u16);

    let res = addr_u16.call(U16(100)).await.unwrap();
    println!("Address<U16> call U16(100): {:?}", res);

    addr_u16.send(U16(200)).unwrap();
    println!("Address<U16> send U16(200): success");

    // Test that Address<U8> can only send U8 messages (type safety)
    // This would cause a compile error if we tried:
    // addr_u8.call(U16(8)).await;  // Compile error: expected U8, found U16

    println!("\n=== All tests completed ===");
}
