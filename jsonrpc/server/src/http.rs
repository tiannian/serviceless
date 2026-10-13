use std::{collections::HashMap, convert::Infallible};

use futures_util::StreamExt;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::{
    body::{Bytes, Frame, Incoming},
    header::CONTENT_TYPE,
    Method, Request, Response, StatusCode, Version,
};
use hyper_tungstenite::tungstenite::protocol::WebSocketConfig;
use serde_json::{json, Value};

use crate::Server;

impl Server {
    pub(super) async fn handle_http(
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
