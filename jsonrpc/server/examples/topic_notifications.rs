//! Run with `cargo run -p serviceless-jsonrpc-server --example topic_notifications`.
//! Then use the SSE POST requests in the README to see per-user notifications.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serviceless::{service, Context, Handler, Message, Service, Topic, TopicEndpoint, TopicRoutes};
use serviceless_jsonrpc::Router;
use serviceless_jsonrpc_server::Server;

/// The JSON in `rpc.subscribe.params.topic` is deserialized into this topic key.
#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Topic)]
#[topic(item = UserEvent)]
struct UserTopic {
    user: String,
}

#[derive(Clone, Serialize)]
struct UserEvent {
    sequence: u64,
    message: String,
}

#[derive(Default, TopicRoutes)]
struct Feed {
    #[topic]
    events: TopicEndpoint<UserTopic>,
}

#[service]
impl Service for Feed {}

#[derive(Message)]
#[message(result = ())]
struct Publish {
    user: String,
    sequence: u64,
}

#[async_trait]
impl Handler<Publish> for Feed {
    async fn handle(&mut self, message: Publish, ctx: &mut Context<Self>) {
        let user = message.user;
        let event = UserEvent {
            sequence: message.sequence,
            message: format!("update for {user}"),
        };
        ctx.publish_handle()
            .publish(UserTopic { user }, event)
            .expect("feed is running");
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let feed = Feed::default();
    let (address, run) = Context::new(&feed).run(feed);
    tokio::spawn(run);

    // This is a single producer. Each publication selects one exact topic key.
    let publisher = address.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        for sequence in 1.. {
            interval.tick().await;
            let user = if sequence % 2 == 0 { "bob" } else { "alice" };
            if publisher
                .send(Publish {
                    user: user.into(),
                    sequence,
                })
                .is_err()
            {
                break;
            }
        }
    });

    let mut router = Router::new();
    router.topic::<UserTopic>("user.updated", address);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("JSON-RPC listening on 127.0.0.1:3000");
    Server::new(router).serve_tcp(listener).await
}
