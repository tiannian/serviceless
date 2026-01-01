use async_trait::async_trait;
use futures_util::{
    future::{ready, Ready as FuturesReady},
    stream::{once, Once},
};
use service_channel::oneshot;

use serviceless::{Context, Envelope, Handler, Message, Service};

#[derive(Debug, Default)]
struct ExternalStreamService {
    stream_events: Vec<String>,
}

#[derive(Debug)]
struct StreamEvent {
    payload: String,
    ack: oneshot::Sender<()>,
}

impl Message for StreamEvent {
    type Result = ();
}

#[async_trait]
impl Handler<StreamEvent> for ExternalStreamService {
    async fn handle(&mut self, message: StreamEvent, _ctx: &mut Context<Self, Self::Stream>) {
        println!("stream pushed: {}", message.payload);
        self.stream_events.push(message.payload);
        let _ = message.ack.send(());
    }
}

#[derive(Debug)]
struct QueryStreamEvents;

impl Message for QueryStreamEvents {
    type Result = usize;
}

#[async_trait]
impl Handler<QueryStreamEvents> for ExternalStreamService {
    async fn handle(
        &mut self,
        _message: QueryStreamEvents,
        _ctx: &mut Context<Self, Self::Stream>,
    ) -> usize {
        self.stream_events.len()
    }
}

#[async_trait]
impl Service for ExternalStreamService {
    type Stream = Once<FuturesReady<Envelope<Self>>>;

    async fn started(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
        println!("external stream service started");
    }

    async fn stopped(&mut self, _ctx: &mut Context<Self, Self::Stream>) {
        println!("external stream service stopped");
    }
}

#[tokio::main]
async fn main() {
    let (ack_tx, ack_rx) = oneshot::channel();

    let stream = once(ready(Envelope::new(StreamEvent {
        payload: "external stream event".to_string(),
        ack: ack_tx,
    })));

    let ctx = Context::with_stream(stream);

    let (service_addr, future) = ExternalStreamService::default().start_by_context(ctx);
    let service_handle = tokio::spawn(future);

    ack_rx.await.expect("stream ack failed");

    let handled_events = service_addr
        .call(QueryStreamEvents)
        .await
        .expect("service call failed");
    println!("handled stream events: {}", handled_events);
    assert_eq!(handled_events, 1);

    service_addr.close_service();
    service_handle.await.expect("service join failed");
}
