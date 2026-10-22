use std::convert::Infallible;

use async_trait::async_trait;
#[cfg(feature = "tcp")]
use futures_util::{SinkExt, StreamExt};
#[cfg(any(feature = "tcp", all(unix, feature = "unix")))]
use http_body_util::{BodyExt, Full};
#[cfg(any(feature = "tcp", all(unix, feature = "unix")))]
use hyper::{body::Bytes, client::conn, Request, StatusCode};
#[cfg(feature = "tcp")]
use hyper_util::rt::TokioExecutor;
#[cfg(any(feature = "tcp", all(unix, feature = "unix")))]
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serviceless::{
    service, Context, Handler, Message as ActorMessage, Metadata, RoutedTopic, Service, Topic,
    TopicEndpoint,
};
use serviceless_jsonrpc::Router;
use serviceless_jsonrpc_server::Server;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(feature = "tcp")]
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

#[cfg(any(feature = "tcp", all(unix, feature = "unix")))]
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

#[cfg(feature = "tcp")]
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

#[cfg(all(unix, feature = "unix"))]
#[tokio::test]
async fn unix_socket_serves_http() {
    let (server, address, actor_task) = server();
    let path = std::path::PathBuf::from("/tmp").join(format!(
        "sjrpc-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 1_000_000
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

#[cfg(all(unix, feature = "unixlines"))]
#[tokio::test]
async fn unix_socket_serves_line_protocol() {
    let (server, address, actor_task) = server();
    let path = std::path::PathBuf::from("/tmp").join(format!(
        "sjrpl-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 1_000_000
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

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct NumberTopic {
    key: String,
}

impl Topic for NumberTopic {
    type Item = i64;
}

#[derive(Default)]
struct EventActor {
    numbers: TopicEndpoint<NumberTopic>,
}

#[async_trait]
impl Service for EventActor {
    type Stream = serviceless::EmptyStream<Self>;
    type Runtime = serviceless::TokioRuntime;
    fn metadata(&self) -> Metadata<'_> {
        Metadata { name: "events" }
    }
}

impl RoutedTopic<EventActor> for NumberTopic {
    fn endpoint(service: &mut EventActor) -> &mut TopicEndpoint<Self> {
        &mut service.numbers
    }
}

#[derive(Serialize)]
struct Publish {
    key: String,
    value: i64,
}
impl ActorMessage for Publish {
    type Result = ();
}

#[async_trait]
impl Handler<Publish> for EventActor {
    async fn handle(&mut self, msg: Publish, ctx: &mut Context<Self>) {
        ctx.publish_handle()
            .publish(NumberTopic { key: msg.key }, msg.value)
            .unwrap();
    }
}

fn event_server() -> (
    Server,
    serviceless::ServiceAddress<EventActor>,
    tokio::task::JoinHandle<()>,
) {
    let actor = EventActor::default();
    let (address, run) = Context::new(&actor).run(actor);
    let task = tokio::spawn(run);
    let mut router = Router::new();
    router.topic::<NumberTopic>("number.changed", address.clone());
    (Server::new(router), address, task)
}

#[cfg(feature = "tcp")]
#[tokio::test]
async fn websocket_allows_one_subscription() {
    let (server, address, actor_task) = event_server();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_task = tokio::spawn(async move { server.serve_tcp(listener).await });
    let (mut socket, _) = connect_async(format!("ws://{addr}/")).await.unwrap();
    socket.send(Message::text(json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"a"}},"id":1}).to_string())).await.unwrap();
    let reply: Value =
        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(reply, json!({"jsonrpc":"2.0","result":true,"id":1}));
    socket.send(Message::text(json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"b"}},"id":2}).to_string())).await.unwrap();
    let reply: Value =
        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(
        reply,
        json!({"jsonrpc":"2.0","error":{"code":-32602,"message":"Already subscribed"},"id":2})
    );
    address
        .send(Publish {
            key: "a".into(),
            value: 7,
        })
        .unwrap();
    let event: Value =
        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(
        event,
        json!({"jsonrpc":"2.0","method":"number.changed","params":7})
    );
    socket
        .send(Message::text(
            json!({"jsonrpc":"2.0","method":"rpc.unsubscribe","params":{},"id":3}).to_string(),
        ))
        .await
        .unwrap();
    let reply: Value =
        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(
        reply,
        json!({"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":3})
    );
    socket.close(None).await.unwrap();
    server_task.abort();
    address.close_service();
    actor_task.await.unwrap();
}

#[cfg(feature = "tcp")]
#[tokio::test]
async fn sse_streams_topic_notifications() {
    let (server, address, actor_task) = event_server();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_task = tokio::spawn(async move { server.serve_tcp(listener).await });
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut client, connection) = conn::http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let get_response = client
        .send_request(
            Request::get("http://localhost/")
                .body(Full::new(Bytes::new()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::METHOD_NOT_ALLOWED);
    drop(get_response);
    let old_endpoint = client
        .send_request(
            Request::post("http://localhost/events")
                .body(Full::new(Bytes::from_static(
                    b"{\"jsonrpc\":\"2.0\",\"method\":\"rpc.subscribe\",\"id\":1}",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_endpoint.status(), StatusCode::NOT_FOUND);
    drop(old_endpoint);
    for (request, code, id) in [
        (
            json!({"method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"a"}},"id":1}),
            -32600,
            json!(1),
        ),
        (
            json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed"},"id":{}}),
            -32600,
            Value::Null,
        ),
        (
            json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"missing"},"id":2}),
            -32601,
            json!(2),
        ),
        (
            json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"topic":{"key":"a"}},"id":4}),
            -32602,
            json!(4),
        ),
        (
            json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"wrong":"a"}},"id":3}),
            -32602,
            json!(3),
        ),
    ] {
        let response = client
            .send_request(
                Request::post("http://localhost/")
                    .body(Full::new(Bytes::from(request.to_string())))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let event = std::str::from_utf8(&body).unwrap();
        let error: Value =
            serde_json::from_str(event.strip_prefix("data: ").unwrap().trim()).unwrap();
        assert_eq!(error["error"]["code"], code);
        assert_eq!(error["id"], id);
    }
    let response = client
        .send_request(
            Request::post("http://localhost/")
                .body(Full::new(Bytes::from(
                    json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"a"}},"id":1}).to_string(),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body();
    let reply = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(
        reply,
        Bytes::from_static(b"data: {\"id\":1,\"jsonrpc\":\"2.0\",\"result\":true}\n\n")
    );
    address
        .send(Publish {
            key: "a".into(),
            value: 9,
        })
        .unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        frame.into_data().unwrap(),
        Bytes::from_static(
            b"data: {\"jsonrpc\":\"2.0\",\"method\":\"number.changed\",\"params\":9}\n\n"
        )
    );
    drop(body);
    server_task.abort();
    address.close_service();
    actor_task.await.unwrap();
}

#[tokio::test]
async fn line_stream_pushes_topic_notifications() {
    let (server, address, actor_task) = event_server();
    let (mut input_writer, input_reader) = tokio::io::duplex(1024);
    let (output_writer, output_reader) = tokio::io::duplex(1024);
    let server_task =
        tokio::spawn(async move { server.serve_stream(input_reader, output_writer).await });
    input_writer.write_all(format!("{}\n", json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"a"}},"id":1})).as_bytes()).await.unwrap();
    let mut output = BufReader::new(output_reader);
    let mut line = String::new();
    output.read_line(&mut line).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"jsonrpc":"2.0","result":true,"id":1})
    );
    input_writer.write_all(format!("{}\n", json!({"jsonrpc":"2.0","method":"rpc.subscribe","params":{"name":"number.changed","topic":{"key":"b"}},"id":2})).as_bytes()).await.unwrap();
    line.clear();
    output.read_line(&mut line).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"jsonrpc":"2.0","error":{"code":-32602,"message":"Already subscribed"},"id":2})
    );
    address
        .send(Publish {
            key: "a".into(),
            value: 11,
        })
        .unwrap();
    line.clear();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        output.read_line(&mut line),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"jsonrpc":"2.0","method":"number.changed","params":11})
    );
    drop(input_writer);
    server_task.await.unwrap().unwrap();
    address.close_service();
    actor_task.await.unwrap();
}
