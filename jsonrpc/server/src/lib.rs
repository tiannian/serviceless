//! Network and stream transports for [`serviceless_jsonrpc::Router`].

use std::{convert::Infallible, io, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    body::{Bytes, Incoming},
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
use serviceless_jsonrpc::Router;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
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

    async fn handle_http(&self, mut request: Request<Incoming>) -> Response<Full<Bytes>> {
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
                    return response;
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
        while let Some(frame) = socket.next().await {
            match frame {
                Ok(Message::Text(text)) => {
                    if text.len() > self.max_message_size {
                        let _ = socket.close(None).await;
                        break;
                    }
                    if let Some(result) = self.router.handle_str(text.as_str()).await {
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
        loop {
            line.clear();
            if read_line_limited(&mut reader, &mut line, self.max_message_size).await? == 0 {
                return Ok(());
            }
            if let Some(result) = self
                .router
                .handle_str(&String::from_utf8_lossy(&line))
                .await
            {
                writer.write_all(result.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
    }

    /// Serve newline-delimited JSON-RPC requests on process stdin and stdout.
    pub async fn serve_stdio(&self) -> io::Result<()> {
        self.serve_stream(tokio::io::stdin(), tokio::io::stdout())
            .await
    }
}

fn response(status: StatusCode, body: Bytes) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(body))
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
