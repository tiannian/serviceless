use async_trait::async_trait;
use futures_util::{
    future::{ready, Ready as FuturesReady},
    stream::{once, Once},
};
use std::time::Duration;
use tokio::time::sleep;

use serviceless::{Context, Envelope, Handler, Message, Metadata, Service};

#[derive(Debug, Default)]
struct ExternalStreamService {}

#[derive(Debug)]
struct StreamEvent(pub u8);

impl Message for StreamEvent {
    type Result = u8;
}

#[async_trait]
impl Handler<StreamEvent> for ExternalStreamService {
    async fn handle(&mut self, message: StreamEvent, _ctx: &mut Context<Self>) -> u8 {
        println!("stream pushed: {}", message.0);
        message.0
    }
}

#[async_trait]
impl Service for ExternalStreamService {
    type Stream = Once<FuturesReady<Envelope<Self>>>;

    type Error = ();

    fn metadata(&self) -> Metadata<'_> {
        Metadata {
            name: "external_stream_service",
        }
    }

    async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        println!("external stream service started");
        Ok(())
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
        println!("external stream service stopped");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stream = once(ready(Envelope::new(StreamEvent(0x42))));

    let ctx = Context::with_stream(stream);

    let (service_addr, future) = ExternalStreamService::default().start_by_context(ctx);
    let service_handle = tokio::spawn(future);

    sleep(Duration::from_millis(20)).await;

    service_addr.close_service();
    service_handle.await.expect("service join failed").unwrap();
}
