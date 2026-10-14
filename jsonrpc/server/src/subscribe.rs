use serde_json::{json, Value};
use serviceless_jsonrpc::{NotificationStream, Router};

pub(super) struct Subscription {
    pub id: Value,
    pub name: String,
    pub stream: NotificationStream,
}

pub(super) fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","error":{"code":code,"message":message},"id":id})
}

pub(super) fn success(id: Value) -> Value {
    json!({"jsonrpc":"2.0","result":true,"id":id})
}

pub(super) fn notification(name: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":name,"params":params})
}

pub(super) fn subscribe(router: &Router, request: &Value) -> Result<Subscription, Value> {
    let valid_id = request
        .get("id")
        .is_some_and(|id| id.is_string() || id.is_number() || id.is_null());
    let id = if valid_id {
        request["id"].clone()
    } else {
        Value::Null
    };
    if !valid_id
        || request.get("jsonrpc") != Some(&Value::String("2.0".into()))
        || request.get("method").and_then(Value::as_str) != Some("rpc.subscribe")
    {
        return Err(error(id, -32600, "Invalid Request"));
    }
    let Some(params) = request.get("params").and_then(Value::as_object) else {
        return Err(error(id, -32602, "Invalid params"));
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err(error(id, -32602, "Invalid params"));
    };
    let topic_params = params.get("topic").cloned().unwrap_or(Value::Null);
    let stream = router
        .subscribe(name, topic_params)
        .map_err(|error_value| {
            error(
                id.clone(),
                error_value.code,
                if error_value.code == -32601 {
                    "Method not found"
                } else {
                    "Invalid params"
                },
            )
        })?;
    Ok(Subscription {
        id,
        name: name.to_owned(),
        stream,
    })
}
