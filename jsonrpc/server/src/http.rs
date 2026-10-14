use std::convert::Infallible;

use futures_util::{stream, StreamExt};
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::{
    body::{Bytes, Frame, Incoming},
    header::CONTENT_TYPE,
    Method, Request, Response, StatusCode, Version,
};
use hyper_tungstenite::tungstenite::protocol::WebSocketConfig;
use serde_json::{json, Value};

use crate::subscribe::{error, subscribe};
use crate::Server;

impl Server {
    pub(super) async fn handle_http(
        &self,
        mut request: Request<Incoming>,
    ) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
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
        if serde_json::from_slice::<Value>(&body)
            .is_ok_and(|value| value.get("method").and_then(Value::as_str) == Some("rpc.subscribe"))
        {
            return self.handle_sse(&body);
        }
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

    fn handle_sse(&self, body: &[u8]) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
        let request = match serde_json::from_slice::<Value>(body) {
            Ok(request) => request,
            Err(_) => return sse_error(error(Value::Null, -32700, "Parse error")),
        };
        let subscription = match subscribe(&self.router, &request) {
            Ok(subscription) => subscription,
            Err(error) => return sse_error(error),
        };
        let reply = json!({"jsonrpc":"2.0","result":true,"id":subscription.id});
        let first = stream::once(async move { sse_frame(reply) });
        let name = subscription.name;
        let events = subscription
            .stream
            .map(move |params| sse_frame(json!({"jsonrpc":"2.0","method":name,"params":params})));
        let stream = first.chain(events);
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("cache-control", "no-cache")
            .body(StreamBody::new(stream).boxed_unsync())
            .expect("valid SSE response")
    }
}

fn sse_frame(value: Value) -> Result<Frame<Bytes>, Infallible> {
    Ok(Frame::data(Bytes::from(format!("data: {value}\n\n"))))
}

fn sse_error(value: Value) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-cache")
        .body(Full::new(Bytes::from(format!("data: {value}\n\n"))).boxed_unsync())
        .expect("valid SSE error response")
}

fn response(status: StatusCode, body: Bytes) -> Response<UnsyncBoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .body(Full::new(body).boxed_unsync())
        .expect("valid response")
}
