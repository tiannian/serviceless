use async_trait::async_trait;
use serviceless::{Context, EmptyStream, Handler, Message, Metadata, Service};

#[derive(Debug, Default)]
pub struct Service0 {}

#[async_trait]
impl Service for Service0 {
    type Stream = EmptyStream<Self>;

    type Error = ();

    fn metadata(&self) -> Metadata<'_> {
        Metadata { name: "service0" }
    }

    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        println!("Started");
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        println!("Stopped");
        Ok(())
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

    let ctx = Context::new(&srv);

    let (service_addr, future) = ctx.run(srv);
    let service_handle = tokio::spawn(future);

    // Test ServiceAddress with multiple message types
    println!("=== Testing ServiceAddress ===");
    let res = service_addr.call(U8(8)).await.unwrap();
    println!("ServiceAddress call U8(8): {:?}", res);

    let res = service_addr.call(U16(8)).await.unwrap();
    println!("ServiceAddress call U16(8): {:?}", res);

    // Test close_service method
    println!("\n=== Testing close_service ===");
    assert!(
        !service_addr.is_stop(),
        "Service should not be stopped before close_service"
    );
    service_addr.close_service();
    assert!(
        !service_addr.is_stop(),
        "Service should not be stopped immediately after close_service (still running)"
    );
    println!("close_service called, service still running");

    // Wait for the service future to complete, which will call stopped hook
    println!("Waiting for service to stop and call stopped hook...");
    service_handle.await.expect("service join failed").unwrap();
    assert!(
        service_addr.is_stop(),
        "Service should be stopped after the service future completes"
    );
    println!("Service future completed, stopped hook should have been called");

    // Verify that sending messages after close fails
    let result = service_addr.send(U8(30));
    assert!(
        result.is_err(),
        "Sending message after close_service should fail"
    );
    println!("Verified: sending message after close_service fails as expected");

    println!("\n=== All tests completed ===");
}
