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

/// Generic message that requires Debug trait bound
#[derive(Debug)]
pub struct GenericMessage<T: std::fmt::Debug + Send + 'static> {
    pub data: T,
}

impl<T: std::fmt::Debug + Send + 'static> Message for GenericMessage<T> {
    type Result = u8;
}

#[async_trait]
impl<T: std::fmt::Debug + Send + 'static> Handler<GenericMessage<T>> for Service0 {
    async fn handle(&mut self, message: GenericMessage<T>, _ctx: &mut Context<Self>) -> u8 {
        println!("Received generic message: {:?}", message);
        1
    }
}

#[tokio::main]
async fn main() {
    let srv = Service0::default();

    let ctx = Context::new();

    let (service_addr, future) = srv.start_by_context(ctx);
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
    service_handle.await.expect("service join failed").unwrap();

    println!("\n=== All tests completed ===");
}
