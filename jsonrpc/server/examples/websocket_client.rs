//! Run the counter example first, then run this example in another terminal.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut socket, _) = connect_async("ws://127.0.0.1:3000/").await?;
    // Notifications have no response. The following call observes the update.
    socket
        .send(Message::text(
            json!({"jsonrpc":"2.0","method":"counter.add","params":{"amount":2}}).to_string(),
        ))
        .await?;
    socket
        .send(Message::text(
            json!([
                {"jsonrpc":"2.0","method":"counter.add","params":{"amount":3},"id":1},
                {"jsonrpc":"2.0","method":"counter.get","params":{},"id":2}
            ])
            .to_string(),
        ))
        .await?;

    // Answer control frames while waiting for the JSON-RPC response.
    while let Some(frame) = socket.next().await {
        match frame? {
            Message::Text(text) => {
                let response: Value = serde_json::from_str(&text)?;
                println!("{}", serde_json::to_string_pretty(&response)?);
                break;
            }
            Message::Ping(payload) => socket.send(Message::Pong(payload)).await?,
            Message::Close(_) => return Err("server closed before responding".into()),
            _ => {}
        }
    }
    socket.close(None).await?;
    Ok(())
}
