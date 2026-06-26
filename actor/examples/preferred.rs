use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, timeout};

use serviceless::{Context, EmptyStream, Handler, Message, Metadata, ReplyHandle, Service};

#[derive(Default)]
struct PreferredService;

#[async_trait]
impl Service for PreferredService {
    type Stream = EmptyStream<Self>;

    type Error = ();

    fn metadata(&self) -> Metadata<'_> {
        Metadata {
            name: "preferred_service",
        }
    }
}

struct SlowDouble(pub u32);

impl Message for SlowDouble {
    type Result = u32;
}

struct Ping;

impl Message for Ping {
    type Result = &'static str;
}

#[async_trait]
impl Handler<SlowDouble> for PreferredService {
    async fn handle(&mut self, message: SlowDouble, _ctx: &mut Context<Self>) -> u32 {
        message.0 * 2
    }

    async fn handle_preferred(
        &mut self,
        message: SlowDouble,
        _ctx: &mut Context<Self>,
        handle: ReplyHandle<SlowDouble>,
    ) {
        tokio::spawn(async move {
            sleep(Duration::from_millis(200)).await;
            let _ = handle.send(message.0 * 2);
        });
    }
}

#[async_trait]
impl Handler<Ping> for PreferredService {
    async fn handle(&mut self, _message: Ping, _ctx: &mut Context<Self>) -> &'static str {
        "pong"
    }
}

#[tokio::main]
async fn main() {
    let service = PreferredService;
    let (addr, run) = Context::new(&service).run(service);
    let service_handle = tokio::spawn(run);

    let addr_for_slow = addr.clone();
    let slow = tokio::spawn(async move { addr_for_slow.call(SlowDouble(21)).await.unwrap() });

    // The slow preferred call is waiting in a spawned task, but mailbox can still process Ping.
    let pong = timeout(Duration::from_millis(50), addr.call(Ping))
        .await
        .expect("ping should not be blocked by preferred work")
        .expect("ping call should succeed");
    println!("quick reply while SlowDouble is pending: {pong}");

    let out = slow.await.expect("join preferred call");
    println!("slow preferred reply: {out}");

    addr.close_service();
    service_handle.await.expect("service join failed").unwrap();
}
