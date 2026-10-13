//! Transport-independent JSON-RPC 2.0 routing to typed actor messages.

use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use serviceless::{Error as ActorError, Handler, Message, Service, ServiceAddress};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An actor address capable of handling a particular message type.
pub trait RpcAddress<M: Message>: Send + Sync {
    fn call_rpc(&self, message: M) -> BoxFuture<'_, Result<M::Result, ActorError>>;
    fn send_rpc(&self, message: M) -> Result<(), ActorError>;
}

impl<S, M> RpcAddress<M> for ServiceAddress<S>
where
    S: Service + Handler<M>,
    M: Message + Send + 'static,
    M::Result: Send,
    ServiceAddress<S>: Send + Sync,
{
    fn call_rpc(&self, message: M) -> BoxFuture<'_, Result<M::Result, ActorError>> {
        Box::pin(self.call(message))
    }

    fn send_rpc(&self, message: M) -> Result<(), ActorError> {
        self.send(message)
    }
}

trait ErasedMethod: Send + Sync {
    fn invoke(
        &self,
        params: Value,
        notification: bool,
    ) -> BoxFuture<'_, Result<Option<Value>, RpcError>>;
}

struct TypedMethod<M, A> {
    address: A,
    _message: PhantomData<fn(M)>,
}

impl<M, A> ErasedMethod for TypedMethod<M, A>
where
    M: Message + DeserializeOwned + Send + 'static,
    M::Result: Serialize + Send,
    A: RpcAddress<M> + 'static,
{
    fn invoke(
        &self,
        params: Value,
        notification: bool,
    ) -> BoxFuture<'_, Result<Option<Value>, RpcError>> {
        Box::pin(async move {
            let message: M =
                serde_json::from_value(params).map_err(|_| RpcError::invalid_params())?;
            if notification {
                self.address
                    .send_rpc(message)
                    .map_err(|_| RpcError::internal())?;
                Ok(None)
            } else {
                let result = self
                    .address
                    .call_rpc(message)
                    .await
                    .map_err(|_| RpcError::internal())?;
                let value = serde_json::to_value(result).map_err(|_| RpcError::internal())?;
                Ok(Some(value))
            }
        })
    }
}

#[derive(Debug)]
struct RpcError {
    code: i32,
    message: &'static str,
}

impl RpcError {
    fn invalid_params() -> Self {
        Self {
            code: -32602,
            message: "Invalid params",
        }
    }
    fn internal() -> Self {
        Self {
            code: -32603,
            message: "Internal error",
        }
    }
}

fn error(code: i32, message: &'static str, id: Value) -> Value {
    json!({"jsonrpc": "2.0", "error": {"code": code, "message": message}, "id": id})
}

/// A JSON-RPC 2.0 method table. It does not own a network listener or transport.
#[derive(Default)]
pub struct Router {
    methods: HashMap<String, Arc<dyn ErasedMethod>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a typed actor message under an exact JSON-RPC method name.
    /// Registration of an existing name replaces the previous method.
    pub fn method<M>(
        &mut self,
        name: impl Into<String>,
        address: impl RpcAddress<M> + 'static,
    ) -> &mut Self
    where
        M: Message + DeserializeOwned + Send + 'static,
        M::Result: Serialize + Send,
    {
        self.methods.insert(
            name.into(),
            Arc::new(TypedMethod::<M, _> {
                address,
                _message: PhantomData,
            }),
        );
        self
    }

    /// Handle one JSON text and return a response, or `None` for notifications.
    pub async fn handle_str(&self, input: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(input) {
            Ok(value) => self.handle_value(value).await,
            Err(_) => Some(error(-32700, "Parse error", Value::Null)),
        };
        response.map(|value| value.to_string())
    }

    /// Handle a parsed JSON-RPC request or batch.
    pub async fn handle_value(&self, value: Value) -> Option<Value> {
        match value {
            Value::Array(items) if items.is_empty() => {
                Some(error(-32600, "Invalid Request", Value::Null))
            }
            Value::Array(items) => {
                let mut responses = Vec::new();
                for item in items {
                    if let Some(response) = self.handle_one(item).await {
                        responses.push(response);
                    }
                }
                if responses.is_empty() {
                    None
                } else {
                    Some(Value::Array(responses))
                }
            }
            other => self.handle_one(other).await,
        }
    }

    async fn handle_one(&self, value: Value) -> Option<Value> {
        let Value::Object(mut request) = value else {
            return Some(error(-32600, "Invalid Request", Value::Null));
        };
        let valid = request.get("jsonrpc") == Some(&Value::String("2.0".into()))
            && request.get("method").is_some_and(Value::is_string)
            && request
                .get("params")
                .is_none_or(|v| v.is_object() || v.is_array())
            && request
                .get("id")
                .is_none_or(|v| v.is_string() || v.is_number() || v.is_null());
        if !valid {
            return Some(error(-32600, "Invalid Request", Value::Null));
        }

        let method = request
            .remove("method")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let id = request.remove("id");
        let notification = id.is_none();
        let params = request.remove("params").unwrap_or(Value::Null);
        let outcome = match self.methods.get(&method) {
            Some(handler) => handler.invoke(params, notification).await,
            None => Err(RpcError {
                code: -32601,
                message: "Method not found",
            }),
        };
        if notification {
            return None;
        }
        let id = id.unwrap();
        Some(match outcome {
            Ok(Some(result)) => json!({"jsonrpc": "2.0", "result": result, "id": id}),
            Ok(None) => error(-32603, "Internal error", id),
            Err(err) => error(err.code, err.message, id),
        })
    }
}
