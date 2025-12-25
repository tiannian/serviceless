use async_trait::async_trait;
use serviceless::{Context, Handler, Message, Service};

#[derive(Debug, Default)]
pub struct Service0 {}

#[async_trait]
impl Service for Service0 {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Started")
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("Stopped")
    }
}

/// Generic message that requires Debug trait bound
#[derive(Debug)]
pub struct GenericMessage<T: std::fmt::Debug> {
    pub data: T,
}

impl<T: std::fmt::Debug> Message for GenericMessage<T> {
    type Result = u8;
}

#[async_trait]
impl<T: std::fmt::Debug + Send + 'static> Handler<GenericMessage<T>> for Service0 {
    async fn handler(&mut self, message: GenericMessage<T>, _ctx: &mut Context<Self>) -> u8 {
        println!("Received generic message: {:?}", message);
        1
    }
}

#[tokio::main]
async fn main() {
    let srv = Service0::default();

    let (service_addr, future) = srv.start();
    let service_handle = tokio::spawn(future);

    // Test with different types
    println!("=== Testing GenericMessage with String ===");
    let msg1 = GenericMessage {
        data: "Hello".to_string(),
    };
    let res = service_addr.call(msg1).await.unwrap();
    println!("Result: {}", res);

    println!("\n=== Testing GenericMessage with i32 ===");
    let msg2 = GenericMessage { data: 42 };
    let res = service_addr.call(msg2).await.unwrap();
    println!("Result: {}", res);

    println!("\n=== Testing GenericMessage with Vec ===");
    let msg3 = GenericMessage {
        data: vec![1, 2, 3],
    };
    let res = service_addr.call(msg3).await.unwrap();
    println!("Result: {}", res);

    // Close service
    println!("\n=== Closing service ===");
    service_addr.close_service();
    service_handle.await.unwrap();

    println!("\n=== All tests completed ===");
}
