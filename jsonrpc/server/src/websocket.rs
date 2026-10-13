use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::{tungstenite::Message, HyperWebsocket};
use tokio::sync::mpsc;

use crate::{session::Session, Server};

impl Server {
    pub(super) async fn serve_websocket(&self, socket: HyperWebsocket) {
        let Ok(mut socket) = socket.await else { return };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut session = Session::new(self.router.clone(), tx);
        loop {
            let frame = tokio::select! {
                frame = socket.next() => frame,
                Some(message) = rx.recv() => {
                    if socket.send(Message::text(message)).await.is_err() { break; }
                    continue;
                }
            };
            let Some(frame) = frame else { break };
            match frame {
                Ok(Message::Text(text)) => {
                    if text.len() > self.max_message_size {
                        let _ = socket.close(None).await;
                        break;
                    }
                    if let Some(result) = session.handle(text.as_str()).await {
                        if socket.send(Message::text(result)).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}
