//! Stateful JSON-RPC methods over stdio or HTTP/WebSocket (requires tcp).
//! See README.md in this directory for requests and expected responses.

use async_trait::async_trait;
use serde::Deserialize;
use serviceless::{service, Context, Handler, Message, Service};
use serviceless_jsonrpc::{JsonRpcError, Router};
use serviceless_jsonrpc_server::Server;

#[derive(Default)]
struct Counter(i64);

#[service]
impl Service for Counter {}

#[derive(Deserialize, Message)]
#[message(result = Result<i64, JsonRpcError>)]
struct Add {
    amount: i64,
}

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, message: Add, _: &mut Context<Self>) -> Result<i64, JsonRpcError> {
        // Reject overflow without changing the actor's state.
        self.0 = self
            .0
            .checked_add(message.amount)
            .ok_or_else(|| JsonRpcError::new(1001, "Counter overflow"))?;
        Ok(self.0)
    }
}

// An empty object (`params: {}`) deserializes into this message.
#[derive(Deserialize, Message)]
#[message(result = Result<i64, JsonRpcError>)]
struct Get {}

#[async_trait]
impl Handler<Get> for Counter {
    async fn handle(&mut self, _: Get, _: &mut Context<Self>) -> Result<i64, JsonRpcError> {
        Ok(self.0)
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let transport = std::env::args().nth(1).unwrap_or_else(|| {
        if cfg!(feature = "tcp") {
            "http"
        } else {
            "stdio"
        }
        .into()
    });
    if !matches!(transport.as_str(), "http" | "stdio") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: counter [http|stdio]",
        ));
    }

    let counter = Counter::default();
    let (address, run) = Context::new(&counter).run(counter);
    let actor = tokio::spawn(run);
    let mut router = Router::new();
    router.method::<Add>("counter.add", address.clone());
    router.method::<Get>("counter.get", address.clone());
    let server = Server::new(router);

    let result = if transport == "stdio" {
        // Keep stdout exclusively for JSON-RPC responses.
        server.serve_stdio().await
    } else {
        #[cfg(feature = "tcp")]
        {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
            eprintln!("JSON-RPC HTTP/WebSocket listening on 127.0.0.1:3000");
            server.serve_tcp(listener).await
        }
        #[cfg(not(feature = "tcp"))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "HTTP requires the tcp feature",
            ))
        }
    };
    address.close_service();
    actor.await?;
    result
}
