use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::{Error, Result};

use http::Uri;
use jsonwebtoken::EncodingKey;
use serde::{Deserialize, Serialize};

use super::{RpcResponse, RpcResponseBatch};

/// JSONRPC Client
#[derive(Clone)]
pub struct RpcClient {
    id: Arc<AtomicU64>,
    url: Uri,
    jwt_key: Option<EncodingKey>,
}

impl RpcClient {
    pub fn new(url: &str, jwt: Option<&[u8]>) -> Result<Self> {
        let url = url.parse()?;

        let jwt_key = jwt.map(EncodingKey::from_secret);

        Ok(Self {
            id: Arc::new(AtomicU64::new(0)),
            url,
            jwt_key,
        })
    }

    pub async fn call<Req, Resp>(&mut self, req: Req) -> Result<RpcResponse<Resp>>
    where
        Req: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let id = self.id.fetch_add(1, Ordering::AcqRel);

        let r = utils::build_request_value(req, id)?;
        let (status_code, bytes) = utils::request(&self.url, &[r], self.jwt_key.as_ref()).await?;

        if status_code.is_success() {
            let resp: RpcResponse<Resp> = serde_json::from_slice(&bytes)?;

            Ok(resp)
        } else {
            Err(Error::NotSuccessCode(status_code))
        }
    }

    pub async fn multi_call<Req, Resp>(
        &mut self,
        requests: &[Req],
    ) -> Result<RpcResponseBatch<Resp>>
    where
        Req: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let mut reqs = Vec::with_capacity(requests.len());

        for req in requests {
            let id = self.id.fetch_add(1, Ordering::AcqRel);

            let r = utils::build_request_value(req, id)?;
            reqs.push(r);
        }

        let (status_code, bytes) = utils::request(&self.url, &reqs, self.jwt_key.as_ref()).await?;

        if status_code.is_success() {
            let resp: RpcResponseBatch<Resp> = serde_json::from_slice(&bytes)?;

            Ok(resp)
        } else {
            Err(Error::NotSuccessCode(status_code))
        }
    }
}

mod utils {
    use http::{HeaderValue, Request, StatusCode, Uri};
    use hyper::{
        body::{self, Bytes},
        Body, Client,
    };
    use jsonwebtoken::EncodingKey;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use time::OffsetDateTime;

    use crate::{Error, Result};

    #[derive(Debug, Serialize, Deserialize)]
    struct JWTClaim {
        pub iat: i64,
    }

    pub fn build_jwt(key: &EncodingKey) -> Result<String> {
        let time = OffsetDateTime::now_utc();
        let claim = JWTClaim {
            iat: time.unix_timestamp(),
        };

        let header = jsonwebtoken::Header::default();

        let jwt = jsonwebtoken::encode(&header, &claim, key)?;

        Ok(jwt)
    }

    pub fn build_request_value<R>(req: R, id: u64) -> Result<Value>
    where
        R: Serialize,
    {
        let mut r = serde_json::to_value(req)?;
        let o = r.as_object_mut().ok_or(Error::WrongFormatOfRequest)?;

        o.insert("id".to_string(), id.into());
        o.insert("jsonrpc".to_string(), "2.0".into());

        log::debug!("Request JSONRpc method: {:?}, id: {}", r.get("method"), id);

        Ok(r)
    }

    pub async fn request(
        uri: &Uri,
        req: &[Value],
        key: Option<&EncodingKey>,
    ) -> Result<(StatusCode, Bytes)> {
        let req_body = serde_json::to_string(&req)?;

        let client = Client::new();
        let mut req = Request::post(uri);
        if let Some(h) = req.headers_mut() {
            h.insert("content-type", HeaderValue::from_static("application/json"));

            if let Some(key) = key {
                let token = build_jwt(key)?;
                h.insert(
                    "Authorization",
                    HeaderValue::from_str(&format!("Bearer {}", token))?,
                );
            }
        }

        let req = req.body(Body::from(req_body))?;
        let response = client.request(req).await?;
        let status_code = response.status();
        let bytes = body::to_bytes(response.into_body()).await?;

        Ok((status_code, bytes))
    }
}
