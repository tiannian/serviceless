use std::convert::Infallible;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, client::conn, Request, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use serde::Deserialize;
use serde_json::{json, Value};
use serviceless::{service, Context, Handler, Message as ActorMessage, Service};
use serviceless_jsonrpc::Router;
use serviceless_jsonrpc_server::Server;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_tungstenite::{connect_async, tungstenite::Message};

struct Counter(i64);

#[service]
impl Service for Counter {}

#[derive(Deserialize)]
struct Add {
    amount: i64,
}

impl ActorMessage for Add {
    type Result = Result<i64, Infallible>;
}

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, msg: Add, _: &mut Context<Self>) -> Result<i64, Infallible> {
        self.0 += msg.amount;
        Ok(self.0)
    }
}

fn server() -> (
    Server,
    serviceless::ServiceAddress<Counter>,
    tokio::task::JoinHandle<()>,
) {
    let actor = Counter(0);
    let (address, run) = Context::new(&actor).run(actor);
    let task = tokio::spawn(run);
    let mut router = Router::new();
    router.method::<Add>("add", address.clone());
    (Server::new(router), address, task)
}

fn request(amount: i64, id: i64) -> String {
    json!({"jsonrpc":"2.0","method":"add","params":{"amount":amount},"id":id}).to_string()
}

async fn http1_request<I>(io: I, amount: i64) -> (StatusCode, String)
where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut client, connection) = conn::http1::handshake(TokioIo::new(io)).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let response = client
        .send_request(
            Request::post("http://localhost/")
                .body(Full::new(Bytes::from(request(amount, 1))))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn tcp_serves_http1_http2_and_websocket() {
    let (server, address, actor_task) = server();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_task = tokio::spawn(async move { server.serve_tcp(listener).await });

    let (status, body) =
        http1_request(tokio::net::TcpStream::connect(addr).await.unwrap(), 2).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"jsonrpc":"2.0","result":2,"id":1})
    );

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut client, connection) =
        conn::http2::handshake(TokioExecutor::new(), TokioIo::new(stream))
            .await
            .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let response = client
        .send_request(
            Request::post("http://localhost/")
                .body(Full::new(Bytes::from(request(3, 2))))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"jsonrpc":"2.0","result":5,"id":2})
    );

    let response = client
        .send_request(
            Request::post("http://localhost/")
                .body(Full::new(Bytes::from(
                    json!({"jsonrpc":"2.0","method":"add","params":{"amount":1}}).to_string(),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .is_empty());

    let (mut socket, _) = connect_async(format!("ws://{addr}/")).await.unwrap();
    socket.send(Message::text(request(4, 3))).await.unwrap();
    let frame = socket.next().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(frame.to_text().unwrap()).unwrap(),
        json!({"jsonrpc":"2.0","result":10,"id":3})
    );
    socket.close(None).await.unwrap();

    server_task.abort();
    address.close_service();
    actor_task.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_serves_http() {
    let (server, address, actor_task) = server();
    let path = std::path::PathBuf::from("/tmp").join(format!(
        "sjrpc-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() % 1_000_000
    ));
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server_task = tokio::spawn(async move { server.serve_unix(listener).await });
    let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
    let (status, body) = http1_request(stream, 7).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"jsonrpc":"2.0","result":7,"id":1})
    );

    server_task.abort();
    address.close_service();
    actor_task.await.unwrap();
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_serves_line_protocol() {
    let (server, address, actor_task) = server();
    let path = std::path::PathBuf::from("/tmp").join(format!(
        "sjrpl-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() % 1_000_000
    ));
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let server_task = tokio::spawn(async move { server.serve_unix_lines(listener).await });
    let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    writer
        .write_all(format!("{}\n", request(8, 1)).as_bytes())
        .await
        .unwrap();
    let mut line = String::new();
    BufReader::new(reader).read_line(&mut line).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"jsonrpc":"2.0","result":8,"id":1})
    );

    server_task.abort();
    address.close_service();
    actor_task.await.unwrap();
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn line_stream_serves_calls_and_skips_notifications() {
    let (server, address, actor_task) = server();
    let (mut input_writer, input_reader) = tokio::io::duplex(1024);
    let (output_writer, output_reader) = tokio::io::duplex(1024);
    let server_task =
        tokio::spawn(async move { server.serve_stream(input_reader, output_writer).await });

    input_writer
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"add\",\"params\":{\"amount\":2}}\n")
        .await
        .unwrap();
    input_writer
        .write_all(format!("{}\n", request(3, 4)).as_bytes())
        .await
        .unwrap();
    drop(input_writer);
    let mut output = BufReader::new(output_reader);
    let mut line = String::new();
    output.read_line(&mut line).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"jsonrpc":"2.0","result":5,"id":4})
    );
    assert_eq!(output.read_line(&mut line).await.unwrap(), 0);
    server_task.await.unwrap().unwrap();
    address.close_service();
    actor_task.await.unwrap();
}
