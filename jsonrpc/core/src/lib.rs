//! Transport-independent JSON-RPC 2.0 routing to typed actor messages.

use std::{
    collections::HashMap, convert::Infallible, future::Future, marker::PhantomData, pin::Pin,
    sync::Arc,
};

use futures_core::Stream;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
pub use serde_json_ext::Config;
use serviceless::{
    Error as ActorError, Handler, Message, RoutedTopic, Service, ServiceAddress, Topic,
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type NotificationStream = Pin<Box<dyn Stream<Item = Value> + Send>>;

/// An actor address that can subscribe to every publication of a topic key.
pub trait RpcTopicAddress<T: Topic>: Send + Sync {
    fn subscribe_rpc_with_config(
        &self,
        topic: T,
        _config: &Config,
    ) -> Result<NotificationStream, ActorError>
    where
        T::Item: Serialize,
    {
        self.subscribe_rpc(topic)
    }

    fn subscribe_rpc(&self, topic: T) -> Result<NotificationStream, ActorError>
    where
        T::Item: Serialize;
}

impl<S, T> RpcTopicAddress<T> for ServiceAddress<S>
where
    S: Service,
    T: Topic + RoutedTopic<S>,
    T::Item: Serialize,
    ServiceAddress<S>: Send + Sync,
{
    fn subscribe_rpc(&self, topic: T) -> Result<NotificationStream, ActorError> {
        self.subscribe_rpc_with_config(topic, &Config::default())
    }

    fn subscribe_rpc_with_config(
        &self,
        topic: T,
        config: &Config,
    ) -> Result<NotificationStream, ActorError> {
        let stream = self.subscribe_all(topic)?;
        struct Encoded<U>(U, Config);
        impl<U, I> Stream for Encoded<U>
        where
            U: Stream<Item = I> + Unpin,
            I: Serialize,
        {
            type Item = Value;
            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Value>> {
                Pin::new(&mut self.0)
                    .poll_next(cx)
                    .map(|item| item.and_then(|item| serde_json_ext::to_value(&item, &self.1).ok()))
            }
        }
        Ok(Box::pin(Encoded(stream, config.clone())))
    }
}

trait ErasedTopic: Send + Sync {
    fn subscribe(&self, params: Value, config: &Config)
        -> Result<NotificationStream, JsonRpcError>;
}

struct TypedTopic<T, A> {
    address: A,
    _topic: PhantomData<fn(T)>,
}

impl<T, A> ErasedTopic for TypedTopic<T, A>
where
    T: Topic + DeserializeOwned,
    T::Item: Serialize,
    A: RpcTopicAddress<T> + 'static,
{
    fn subscribe(
        &self,
        params: Value,
        config: &Config,
    ) -> Result<NotificationStream, JsonRpcError> {
        let topic = serde_json_ext::from_value(params, config)
            .map_err(|_| JsonRpcError::invalid_params())?;
        self.address
            .subscribe_rpc_with_config(topic, config)
            .map_err(|_| JsonRpcError::internal())
    }
}

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
    fn invoke<'a>(
        &'a self,
        params: Value,
        notification: bool,
        config: &'a Config,
    ) -> BoxFuture<'a, Result<Option<Value>, JsonRpcError>>;
}

struct TypedMethod<M, A> {
    address: A,
    _message: PhantomData<fn(M)>,
}

impl<M, A> ErasedMethod for TypedMethod<M, A>
where
    M: Message + DeserializeOwned + Send + 'static,
    M::Result: RpcResult + Send,
    A: RpcAddress<M> + 'static,
{
    fn invoke<'a>(
        &'a self,
        params: Value,
        notification: bool,
        config: &'a Config,
    ) -> BoxFuture<'a, Result<Option<Value>, JsonRpcError>> {
        Box::pin(async move {
            let message: M = serde_json_ext::from_value(params, config)
                .map_err(|_| JsonRpcError::invalid_params())?;
            if notification {
                self.address
                    .send_rpc(message)
                    .map_err(|_| JsonRpcError::internal())?;
                Ok(None)
            } else {
                let result = self
                    .address
                    .call_rpc(message)
                    .await
                    .map_err(|_| JsonRpcError::internal())?;
                result.into_rpc_result_with_config(config).map(Some)
            }
        })
    }
}

/// Converts a handler's `Result<R, E>` into a JSON-RPC success or error.
/// Implemented for `Result<R, E>` when `R: Serialize` and `E: Into<JsonRpcError>`.
/// Use `Infallible` or `()` as the error type for handlers without business errors.
pub trait RpcResult {
    fn into_rpc_result(self) -> Result<Value, JsonRpcError>;

    fn into_rpc_result_with_config(self, _config: &Config) -> Result<Value, JsonRpcError>
    where
        Self: Sized,
    {
        self.into_rpc_result()
    }
}

impl<R, E> RpcResult for Result<R, E>
where
    R: Serialize,
    E: Into<JsonRpcError>,
{
    fn into_rpc_result(self) -> Result<Value, JsonRpcError> {
        self.into_rpc_result_with_config(&Config::default())
    }

    fn into_rpc_result_with_config(self, config: &Config) -> Result<Value, JsonRpcError> {
        match self {
            Ok(value) => {
                serde_json_ext::to_value(&value, config).map_err(|_| JsonRpcError::internal())
            }
            Err(error) => Err(error.into()),
        }
    }
}

/// A JSON-RPC error returned by an actor handler.
#[derive(Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }
    fn internal() -> Self {
        Self::new(-32603, "Internal error")
    }
}

impl From<Infallible> for JsonRpcError {
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

impl From<()> for JsonRpcError {
    fn from(_: ()) -> Self {
        Self::internal()
    }
}

fn error(code: i32, message: &'static str, id: Value) -> Value {
    error_response(JsonRpcError::new(code, message), id)
}

fn error_response(error: JsonRpcError, id: Value) -> Value {
    let mut details = json!({"code": error.code, "message": error.message});
    if let Some(data) = error.data {
        details["data"] = data;
    }
    json!({"jsonrpc": "2.0", "error": details, "id": id})
}

/// A JSON-RPC 2.0 method table. It does not own a network listener or transport.
#[derive(Default)]
pub struct Router {
    config: Config,
    methods: HashMap<String, Arc<dyn ErasedMethod>>,
    topics: HashMap<String, Arc<dyn ErasedTopic>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure typed parameter decoding, handler results, and topic events.
    /// Applies to all registrations, including those added before this call.
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Expose a typed actor topic under an exact event name.
    pub fn topic<T>(
        &mut self,
        name: impl Into<String>,
        address: impl RpcTopicAddress<T> + 'static,
    ) -> &mut Self
    where
        T: Topic + DeserializeOwned,
        T::Item: Serialize,
    {
        self.topics.insert(
            name.into(),
            Arc::new(TypedTopic::<T, _> {
                address,
                _topic: PhantomData,
            }),
        );
        self
    }

    /// Subscribe to a registered topic key. Transport sessions own the returned stream.
    pub fn subscribe(&self, name: &str, params: Value) -> Result<NotificationStream, JsonRpcError> {
        self.topics
            .get(name)
            .ok_or_else(|| JsonRpcError::new(-32601, "Method not found"))?
            .subscribe(params, &self.config)
    }

    /// Register a typed actor message returning `Result<R, E>` under an exact method name.
    /// `Ok(R)` becomes a JSON-RPC result; `Err(E)` becomes a JSON-RPC error.
    /// Use `Infallible` or `()` for methods without business errors.
    /// Registration of an existing name replaces the previous method.
    pub fn method<M>(
        &mut self,
        name: impl Into<String>,
        address: impl RpcAddress<M> + 'static,
    ) -> &mut Self
    where
        M: Message + DeserializeOwned + Send + 'static,
        M::Result: RpcResult + Send,
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
            Some(handler) => handler.invoke(params, notification, &self.config).await,
            None => Err(JsonRpcError::new(-32601, "Method not found")),
        };
        if notification {
            return None;
        }
        let id = id.unwrap();
        Some(match outcome {
            Ok(Some(result)) => json!({"jsonrpc": "2.0", "result": result, "id": id}),
            Ok(None) => error(-32603, "Internal error", id),
            Err(err) => error_response(err, id),
        })
    }
}
