use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serviceless::{service, Context, Handler, Message, Service, Topic, TopicEndpoint, TopicRoutes};
use serviceless_jsonrpc::{Config, Router};

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = BytesEvent)]
struct BytesTopic {
    #[serde(with = "serde_bytes")]
    key: Vec<u8>,
}

#[derive(Clone, Serialize)]
struct BytesEvent {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

#[derive(Default, TopicRoutes)]
struct Feed {
    #[topic]
    events: TopicEndpoint<BytesTopic>,
}

#[service]
impl Service for Feed {}

#[derive(Message)]
#[message(result = ())]
struct Publish;

#[async_trait]
impl Handler<Publish> for Feed {
    async fn handle(&mut self, _: Publish, ctx: &mut Context<Self>) {
        ctx.publish_handle()
            .publish(
                BytesTopic { key: vec![0, 255] },
                BytesEvent { data: vec![1, 254] },
            )
            .unwrap();
    }
}

#[tokio::test]
async fn topic_uses_router_config_for_subscription_keys_and_events() {
    let feed = Feed::default();
    let (address, run) = Context::new(&feed).run(feed);
    let task = tokio::spawn(run);

    let mut default_router = Router::default();
    default_router.topic::<BytesTopic>("bytes", address.clone());
    let mut default_stream = default_router
        .subscribe("bytes", json!({"key": [0, 255]}))
        .unwrap();

    let mut hex_router = Router::new();
    hex_router.topic::<BytesTopic>("bytes", address.clone());
    let hex_router = hex_router.with_config(Config::default().set_bytes_hex().enable_hex_prefix());
    let mut hex_stream = hex_router
        .subscribe("bytes", json!({"key": "0x00ff"}))
        .unwrap();
    assert_eq!(
        hex_router
            .subscribe("bytes", json!({"key": "0xinvalid"}))
            .err()
            .unwrap()
            .code,
        -32602,
    );

    address.call(Publish).await.unwrap();
    // Close the feed so a missing publication ends the stream instead of hanging the test.
    address.close_service();
    task.await.unwrap();

    assert_eq!(
        std::future::poll_fn(|cx| default_stream.as_mut().poll_next(cx)).await,
        Some(json!({"data": [1, 254]})),
    );
    assert_eq!(
        std::future::poll_fn(|cx| hex_stream.as_mut().poll_next(cx)).await,
        Some(json!({"data": "0x01fe"})),
    );
}
