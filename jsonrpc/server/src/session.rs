use std::{collections::HashMap, sync::Arc};

use futures_util::StreamExt;
use serde_json::{json, Value};
use serviceless_jsonrpc::Router;
use tokio::sync::mpsc;

pub(super) struct Session {
    router: Arc<Router>,
    tx: mpsc::UnboundedSender<String>,
    next_id: u64,
    subscriptions: HashMap<u64, tokio::task::JoinHandle<()>>,
}

impl Session {
    pub(super) fn new(router: Arc<Router>, tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            router,
            tx,
            next_id: 1,
            subscriptions: HashMap::new(),
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
        if method != "rpc.subscribe" && method != "rpc.unsubscribe" {
            return self
                .router
                .handle_value(value)
                .await
                .map(|value| value.to_string());
        }
        let id = value.get("id").cloned()?;
        if !(id.is_string() || id.is_number() || id.is_null()) {
            return Some(json!({"jsonrpc":"2.0","error":{"code":-32600,"message":"Invalid Request"},"id":null}).to_string());
        }
        let result = if value.get("jsonrpc") != Some(&Value::String("2.0".into())) {
            Err((-32600, "Invalid Request"))
        } else if !value.get("params").is_some_and(Value::is_object) {
            Err((-32602, "Invalid params"))
        } else if method == "rpc.subscribe" {
            let params = value.get("params").unwrap_or(&Value::Null);
            match params.get("method").and_then(Value::as_str) {
                Some(name) => {
                    let topic_params = params.get("params").cloned().unwrap_or(Value::Null);
                    match self.router.subscribe(name, topic_params) {
                        Ok(mut stream) => {
                            let subscription_id = self.next_id;
                            self.next_id += 1;
                            let tx = self.tx.clone();
                            let name = name.to_owned();
                            let task = tokio::spawn(async move {
                                while let Some(params) = stream.next().await {
                                    let notification = json!({"jsonrpc":"2.0","method":name,"params":{"subscription":subscription_id,"value":params}});
                                    if tx.send(notification.to_string()).is_err() {
                                        break;
                                    }
                                }
                            });
                            self.subscriptions.insert(subscription_id, task);
                            Ok(json!(subscription_id))
                        }
                        Err(error) => Err((
                            error.code,
                            if error.code == -32601 {
                                "Method not found"
                            } else {
                                "Invalid params"
                            },
                        )),
                    }
                }
                None => Err((-32602, "Invalid params")),
            }
        } else {
            match value
                .get("params")
                .and_then(|v| v.get("subscription"))
                .and_then(Value::as_u64)
            {
                Some(subscription_id) => Ok(json!(self
                    .subscriptions
                    .remove(&subscription_id)
                    .is_some_and(|task| {
                        task.abort();
                        true
                    }))),
                None => Err((-32602, "Invalid params")),
            }
        };
        Some(
            match result {
                Ok(result) => json!({"jsonrpc":"2.0","result":result,"id":id}),
                Err((code, message)) => {
                    json!({"jsonrpc":"2.0","error":{"code":code,"message":message},"id":id})
                }
            }
            .to_string(),
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        for (_, task) in self.subscriptions.drain() {
            task.abort();
        }
    }
}
