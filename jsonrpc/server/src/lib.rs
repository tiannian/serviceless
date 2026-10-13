//! Network and stream transports for [`serviceless_jsonrpc::Router`].

use std::{collections::HashMap, convert::Infallible, io, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::{
    body::{Bytes, Frame, Incoming},
    header::CONTENT_TYPE,
    service::service_fn,
    Method, Request, Response, StatusCode, Version,
};
use hyper_tungstenite::{
    tungstenite::{protocol::WebSocketConfig, Message},
    HyperWebsocket,
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto,
};
use serde_json::{json, Value};
use serviceless_jsonrpc::Router;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::warn;

const DEFAULT_MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Serves a JSON-RPC router over HTTP, WebSocket, or newline-framed streams.
#[derive(Clone)]
pub struct Server {
    router: Arc<Router>,
    path: Arc<str>,
    max_message_size: usize,
}

impl Server {
    /// Use `/` as the HTTP and WebSocket endpoint.
    pub fn new(router: Router) -> Self {
        Self {
            router: Arc::new(router),
            path: Arc::from("/"),
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }

    /// Change the HTTP and WebSocket endpoint path.
    pub fn with_path(mut self, path: impl Into<Arc<str>>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the maximum HTTP body, WebSocket text message, or stream line size in bytes.
    pub fn with_max_message_size(mut self, bytes: usize) -> Self {
        self.max_message_size = bytes;
        self
    }

    /// Accept HTTP/1.1, cleartext HTTP/2, and HTTP/1.1 WebSocket on a TCP listener.
    pub async fn serve_tcp(&self, listener: tokio::net::TcpListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone();
            tokio::spawn(async move { server.serve_connection(stream).await });
        }
    }

    /// Accept HTTP/1.1, cleartext HTTP/2, and HTTP/1.1 WebSocket on a Unix listener.
    /// The caller owns the socket path and its cleanup.
    #[cfg(unix)]
    pub async fn serve_unix(&self, listener: tokio::net::UnixListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone();
            tokio::spawn(async move { server.serve_connection(stream).await });
        }
    }

    /// Accept newline-delimited JSON-RPC directly on a Unix listener.
    #[cfg(unix)]
    pub async fn serve_unix_lines(&self, listener: tokio::net::UnixListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone();
            tokio::spawn(async move {
                let (reader, writer) = tokio::io::split(stream);
                if let Err(error) = server.serve_stream(reader, writer).await {
                    warn!(%error, "JSON-RPC Unix stream closed with error");
                }
            });
        }
    }

    async fn serve_connection<I>(&self, stream: I)
    where
        I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let server = self.clone();
        let service = service_fn(move |request| {
            let server = server.clone();
            async move { Ok::<_, Infallible>(server.handle_http(request).await) }
        });
        let builder = auto::Builder::new(TokioExecutor::new());
        if let Err(error) = builder
            .serve_connection_with_upgrades(TokioIo::new(stream), service)
            .await
        {
            warn!(%error, "JSON-RPC connection closed with error");
        }
    }

    async fn handle_http(
        &self,
        mut request: Request<Incoming>,
    ) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
        if request.method() == Method::GET
            && request.uri().path() == format!("{}/events", self.path.trim_end_matches('/'))
        {
            return self.handle_sse(request.uri().query());
        }
        if request.uri().path() != self.path.as_ref() {
            return response(StatusCode::NOT_FOUND, Bytes::new());
        }

        if hyper_tungstenite::is_upgrade_request(&request) {
            if request.version() != Version::HTTP_11 || request.method() != Method::GET {
                return response(StatusCode::BAD_REQUEST, Bytes::new());
            }
            let mut config = WebSocketConfig::default();
            config.max_message_size = Some(self.max_message_size);
            config.max_frame_size = Some(self.max_message_size);
            match hyper_tungstenite::upgrade(&mut request, Some(config)) {
                Ok((response, socket)) => {
                    let server = self.clone();
                    tokio::spawn(async move { server.serve_websocket(socket).await });
                    return response.map(|body| body.boxed_unsync());
                }
                Err(_) => return response(StatusCode::BAD_REQUEST, Bytes::new()),
            }
        }

        if request.method() != Method::POST {
            return response(StatusCode::METHOD_NOT_ALLOWED, Bytes::new());
        }

        let body = match Limited::new(request.into_body(), self.max_message_size)
            .collect()
            .await
        {
            Ok(body) => body.to_bytes(),
            Err(_) => return response(StatusCode::PAYLOAD_TOO_LARGE, Bytes::new()),
        };
        let Some(result) = self
            .router
            .handle_str(&String::from_utf8_lossy(&body))
            .await
        else {
            return response(StatusCode::NO_CONTENT, Bytes::new());
        };
        let mut response = response(StatusCode::OK, Bytes::from(result));
        response.headers_mut().insert(
            CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );
        response
    }

    async fn serve_websocket(&self, socket: HyperWebsocket) {
        let Ok(mut socket) = socket.await else { return };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut session = Session::new(self.router.clone(), tx);
        loop {
            let frame = tokio::select! {
                frame = socket.next() => frame,
                Some(message) = rx.recv() => {
                    if socket.send(Message::text(message)).await.is_err() { break; }
                    continue;
                }
            };
            let Some(frame) = frame else { break };
            match frame {
                Ok(Message::Text(text)) => {
                    if text.len() > self.max_message_size {
                        let _ = socket.close(None).await;
                        break;
                    }
                    if let Some(result) = session.handle(text.as_str()).await {
                        if socket.send(Message::text(result)).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    /// Process one JSON document per input line, writing one response line per call.
    /// Notifications do not produce output. Useful for stdio or custom streams.
    pub async fn serve_stream<R, W>(&self, reader: R, mut writer: W) -> io::Result<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut session = Session::new(self.router.clone(), tx);
        loop {
            let size = tokio::select! {
                size = read_line_limited(&mut reader, &mut line, self.max_message_size) => size?,
                Some(message) = rx.recv() => {
                    writer.write_all(message.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }
            };
            if size == 0 {
                return Ok(());
            }
            if let Some(result) = session.handle(&String::from_utf8_lossy(&line)).await {
                writer.write_all(result.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            line.clear();
        }
    }

    /// Serve newline-delimited JSON-RPC requests on process stdin and stdout.
    pub async fn serve_stdio(&self) -> io::Result<()> {
        self.serve_stream(tokio::io::stdin(), tokio::io::stdout())
            .await
    }

    fn handle_sse(&self, query: Option<&str>) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
        let Some(query) = query else {
            return response(StatusCode::BAD_REQUEST, Bytes::new());
        };
        let pairs: HashMap<_, _> = query
            .split('&')
            .filter_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                Some((key, percent_decode(value)?))
            })
            .collect();
        let Some(method) = pairs.get("method") else {
            return response(StatusCode::BAD_REQUEST, Bytes::new());
        };
        let params = match pairs.get("params") {
            Some(raw) => match serde_json::from_str(raw) {
                Ok(value) => value,
                Err(_) => return response(StatusCode::BAD_REQUEST, Bytes::new()),
            },
            None => Value::Null,
        };
        let stream = match self.router.subscribe(method, params) {
            Ok(stream) => stream,
            Err(_) => return response(StatusCode::BAD_REQUEST, Bytes::new()),
        };
        let method = method.clone();
        let stream = stream.map(move |params| {
            let value = json!({"jsonrpc":"2.0","method":method,"params":params});
            Ok::<_, Infallible>(Frame::data(Bytes::from(format!("data: {value}\n\n"))))
        });
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("cache-control", "no-cache")
            .body(StreamBody::new(stream).boxed_unsync())
            .expect("valid SSE response")
    }
}

struct Session {
    router: Arc<Router>,
    tx: mpsc::UnboundedSender<String>,
    next_id: u64,
    subscriptions: HashMap<u64, tokio::task::JoinHandle<()>>,
}

impl Session {
    fn new(router: Arc<Router>, tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            router,
            tx,
            next_id: 1,
            subscriptions: HashMap::new(),
        }
    }

    async fn handle(&mut self, input: &str) -> Option<String> {
        let Ok(value) = serde_json::from_str::<Value>(input) else {
            return self.router.handle_str(input).await;
        };
        let Some(method) = value.get("method").and_then(Value::as_str) else {
            return self
                .router
                .handle_value(value)
                .await
                .map(|value| value.to_string());
        };
        if method != "rpc.subscribe" && method != "rpc.unsubscribe" {
            return self
                .router
                .handle_value(value)
                .await
                .map(|value| value.to_string());
        }
        let id = value.get("id").cloned()?;
        if !(id.is_string() || id.is_number() || id.is_null()) {
            return Some(json!({"jsonrpc":"2.0","error":{"code":-32600,"message":"Invalid Request"},"id":null}).to_string());
        }
        let result = if value.get("jsonrpc") != Some(&Value::String("2.0".into())) {
            Err((-32600, "Invalid Request"))
        } else if !value.get("params").is_some_and(Value::is_object) {
            Err((-32602, "Invalid params"))
        } else if method == "rpc.subscribe" {
            let params = value.get("params").unwrap_or(&Value::Null);
            match params.get("method").and_then(Value::as_str) {
                Some(name) => {
                    let topic_params = params.get("params").cloned().unwrap_or(Value::Null);
                    match self.router.subscribe(name, topic_params) {
                        Ok(mut stream) => {
                            let subscription_id = self.next_id;
                            self.next_id += 1;
                            let tx = self.tx.clone();
                            let name = name.to_owned();
                            let task = tokio::spawn(async move {
                                while let Some(params) = stream.next().await {
                                    let notification = json!({"jsonrpc":"2.0","method":name,"params":{"subscription":subscription_id,"value":params}});
                                    if tx.send(notification.to_string()).is_err() {
                                        break;
                                    }
                                }
                            });
                            self.subscriptions.insert(subscription_id, task);
                            Ok(json!(subscription_id))
                        }
                        Err(error) => Err((
                            error.code,
                            if error.code == -32601 {
                                "Method not found"
                            } else {
                                "Invalid params"
                            },
                        )),
                    }
                }
                None => Err((-32602, "Invalid params")),
            }
        } else {
            match value
                .get("params")
                .and_then(|v| v.get("subscription"))
                .and_then(Value::as_u64)
            {
                Some(subscription_id) => Ok(json!(self
                    .subscriptions
                    .remove(&subscription_id)
                    .is_some_and(|task| {
                        task.abort();
                        true
                    }))),
                None => Err((-32602, "Invalid params")),
            }
        };
        Some(
            match result {
                Ok(result) => json!({"jsonrpc":"2.0","result":result,"id":id}),
                Err((code, message)) => {
                    json!({"jsonrpc":"2.0","error":{"code":code,"message":message},"id":id})
                }
            }
            .to_string(),
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        for (_, task) in self.subscriptions.drain() {
            task.abort();
        }
    }
}

fn percent_decode(input: &str) -> Option<String> {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => {
                let high = (bytes.next()? as char).to_digit(16)?;
                let low = (bytes.next()? as char).to_digit(16)?;
                output.push(((high << 4) | low) as u8);
            }
            b'+' => output.push(b' '),
            _ => output.push(byte),
        }
    }
    String::from_utf8(output).ok()
}

fn response(status: StatusCode, body: Bytes) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .body(Full::new(body).boxed_unsync())
        .expect("valid response")
}

async fn read_line_limited<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    line: &mut Vec<u8>,
    limit: usize,
) -> io::Result<usize> {
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let count = available
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(count) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON-RPC line exceeds size limit",
            ));
        }
        let complete = available[count - 1] == b'\n';
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if complete {
            return Ok(line.len());
        }
    }
}
