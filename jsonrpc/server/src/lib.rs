//! Network and stream transports for [`serviceless_jsonrpc::Router`].

mod http;
mod session;
mod stream;
mod subscribe;
mod websocket;

use std::{convert::Infallible, io, sync::Arc};

use hyper::service::service_fn;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto,
};
use serviceless_jsonrpc::Router;
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
}
