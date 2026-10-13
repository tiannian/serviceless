use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use serviceless::{service, Context, Handler, Message, Service};
use serviceless_jsonrpc::{JsonRpcError, Router};

struct Counter(i64);

#[service]
impl Service for Counter {}

#[derive(Deserialize)]
struct Add {
    amount: i64,
}
impl Message for Add {
    type Result = i64;
}

#[async_trait]
impl Handler<Add> for Counter {
    async fn handle(&mut self, msg: Add, _: &mut Context<Self>) -> i64 {
        self.0 += msg.amount;
        self.0
    }
}

#[derive(Deserialize)]
struct Get;
impl Message for Get {
    type Result = i64;
}

#[async_trait]
impl Handler<Get> for Counter {
    async fn handle(&mut self, _: Get, _: &mut Context<Self>) -> i64 {
        self.0
    }
}

#[derive(Deserialize)]
struct AddChecked {
    amount: i64,
}

impl Message for AddChecked {
    type Result = Result<i64, CounterError>;
}

struct CounterError;

impl From<CounterError> for JsonRpcError {
    fn from(_: CounterError) -> Self {
        JsonRpcError::new(1001, "Amount must be positive").with_data(json!({"field": "amount"}))
    }
}

#[async_trait]
impl Handler<AddChecked> for Counter {
    async fn handle(
        &mut self,
        msg: AddChecked,
        _: &mut Context<Self>,
    ) -> Result<i64, CounterError> {
        if msg.amount <= 0 {
            return Err(CounterError);
        }
        self.0 += msg.amount;
        Ok(self.0)
    }
}

#[tokio::test]
async fn routes_calls_and_notifications_to_typed_handlers() {
    let counter = Counter(0);
    let (address, run) = Context::new(&counter).run(counter);
    let task = tokio::spawn(run);
    let mut router = Router::new();
    router
        .method::<Add>("counter.add", address.clone())
        .method::<Get>("counter.get", address.clone());

    assert_eq!(
        router
            .handle_value(
                json!({"jsonrpc":"2.0","method":"counter.add","params":{"amount":3},"id":1})
            )
            .await,
        Some(json!({"jsonrpc":"2.0","result":3,"id":1}))
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}}))
            .await,
        None
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"counter.get","id":"next"}))
            .await,
        Some(json!({"jsonrpc":"2.0","result":5,"id":"next"}))
    );

    address.close_service();
    task.await.unwrap();
}

#[tokio::test]
async fn reports_protocol_errors_and_filters_batch_notifications() {
    let counter = Counter(0);
    let (address, run) = Context::new(&counter).run(counter);
    let task = tokio::spawn(run);
    let mut router = Router::new();
    router.method::<Add>("add", address.clone());

    let response: Value = serde_json::from_str(&router.handle_str("{").await.unwrap()).unwrap();
    assert_eq!(
        response,
        json!({"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null})
    );
    assert_eq!(
        router.handle_value(json!([])).await,
        Some(
            json!({"jsonrpc":"2.0","error":{"code":-32600,"message":"Invalid Request"},"id":null})
        )
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"missing","id":null}))
            .await,
        Some(
            json!({"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":null})
        )
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"add","params":{"wrong":1},"id":2}))
            .await,
        Some(json!({"jsonrpc":"2.0","error":{"code":-32602,"message":"Invalid params"},"id":2}))
    );
    assert_eq!(
        router
            .handle_value(json!([
                {"jsonrpc":"2.0","method":"add","params":{"amount":4}},
                {"jsonrpc":"2.0","method":"add","params":{"amount":1},"id":"x"},
                4
            ]))
            .await,
        Some(json!([
            {"jsonrpc":"2.0","result":5,"id":"x"},
            {"jsonrpc":"2.0","error":{"code":-32600,"message":"Invalid Request"},"id":null}
        ]))
    );
    assert_eq!(
        router
            .handle_value(json!([{"jsonrpc":"2.0","method":"add","params":{"amount":1}}]))
            .await,
        None
    );

    address.close_service();
    task.await.unwrap();
}

#[tokio::test]
async fn method_with_error_converts_handler_results_and_preserves_error_data() {
    let counter = Counter(0);
    let (address, run) = Context::new(&counter).run(counter);
    let task = tokio::spawn(run);
    let mut router = Router::new();
    router
        .method_with_error::<AddChecked>("add_checked", address.clone())
        .method::<Get>("get", address.clone());

    assert_eq!(
        router
            .handle_value(
                json!({"jsonrpc":"2.0","method":"add_checked","params":{"amount":3},"id":1})
            )
            .await,
        Some(json!({"jsonrpc":"2.0","result":3,"id":1}))
    );
    assert_eq!(
        router
            .handle_value(
                json!({"jsonrpc":"2.0","method":"add_checked","params":{"amount":0},"id":"bad"})
            )
            .await,
        Some(
            json!({"jsonrpc":"2.0","error":{"code":1001,"message":"Amount must be positive","data":{"field":"amount"}},"id":"bad"})
        )
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"add_checked","params":{"amount":0}}))
            .await,
        None
    );
    assert_eq!(
        router
            .handle_value(json!({"jsonrpc":"2.0","method":"get","id":2}))
            .await,
        Some(json!({"jsonrpc":"2.0","result":3,"id":2}))
    );

    address.close_service();
    task.await.unwrap();
}
