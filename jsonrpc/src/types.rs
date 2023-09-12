use serde::Deserialize;
use serde_json::Value;

use crate::RpcError;

/// JSONRPC Response Batch
pub type RpcResponseBatch<T> = Vec<RpcResponse<T>>;

/// JSONRPC Response
#[derive(Debug, Clone, Deserialize)]
pub struct RpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<RpcError>,
    pub id: Value,
}

impl<T> RpcResponse<T> {
    pub fn into_result(self) -> Result<Option<T>, RpcError> {
        match (self.result, self.error) {
            (Some(v), _) => Ok(Some(v)),
            (None, Some(e)) => Err(e),
            (None, None) => Ok(None),
        }
    }
}
