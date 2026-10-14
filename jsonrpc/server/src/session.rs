use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::Value;
use serviceless_jsonrpc::Router;
use tokio::sync::mpsc;

use crate::subscribe::{error, notification, subscribe, success};

pub(super) struct Session {
    router: Arc<Router>,
    tx: mpsc::UnboundedSender<String>,
    subscription: Option<tokio::task::JoinHandle<()>>,
}

impl Session {
    pub(super) fn new(router: Arc<Router>, tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            router,
            tx,
            subscription: None,
        }
    }

    pub(super) async fn handle(&mut self, input: &str) -> Option<String> {
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
        if method != "rpc.subscribe" {
            return self
                .router
                .handle_value(value)
                .await
                .map(|value| value.to_string());
        }
        if !value
            .get("id")
            .is_some_and(|id| id.is_string() || id.is_number() || id.is_null())
        {
            return value
                .get("id")
                .map(|_| error(Value::Null, -32600, "Invalid Request").to_string());
        }
        if self.subscription.is_some() {
            return Some(error(value["id"].clone(), -32602, "Already subscribed").to_string());
        }
        let subscription = match subscribe(&self.router, &value) {
            Ok(subscription) => subscription,
            Err(error) => return Some(error.to_string()),
        };
        let tx = self.tx.clone();
        let name = subscription.name;
        let mut stream = subscription.stream;
        self.subscription = Some(tokio::spawn(async move {
            while let Some(params) = stream.next().await {
                let event = notification(&name, params);
                if tx.send(event.to_string()).is_err() {
                    break;
                }
            }
        }));
        Some(success(subscription.id).to_string())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(task) = self.subscription.take() {
            task.abort();
        }
    }
}
