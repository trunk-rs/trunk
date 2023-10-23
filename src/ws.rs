use crate::serve;
use axum::extract::ws::{Message, WebSocket};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio_stream::wrappers::WatchStream;

/// (outgoing) communication messages with the websocket
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Reload,
    BuildFailure { reason: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum State {
    #[default]
    Ok,
    Failed {
        reason: String,
    },
}

pub(crate) async fn handle_ws(mut ws: WebSocket, state: Arc<serve::State>) {
    let mut rx = WatchStream::new(state.ws_state.clone());
    tracing::debug!("autoreload websocket opened");

    let mut first = true;

    loop {
        tokio::select! {
            _ = ws.recv() => {
                tracing::debug!("autoreload websocket closed");
                return
            }
            state = rx.next() => {

                let state = match state {
                    Some(state) => state,
                    None => break,
                };

                tracing::debug!("Build state changed: {state:?}");

                let msg = match state {
                    State::Ok if first => {
                        // If the state is ok, and it's the first message we would send, discard it,
                        // as this would cause a reload right after connecting. On the other side,
                        // we want to send out a failed build even after reconnecting.
                        first = false;
                        tracing::debug!("Discarding first reload trigger");
                        None
                    },
                    State::Ok  => Some(ClientMessage::Reload),
                    State::Failed { reason } => Some(ClientMessage::BuildFailure { reason }),
                };

                tracing::debug!("Message to send: {msg:?}");

                if let Some(msg) = msg {
                    if let Ok(text) = serde_json::to_string(&msg) {
                        if let Err(err) = ws.send(Message::Text(text)).await {
                            tracing::info!("autoload websocket failed to send: {err}");
                            break;
                        }
                    }
                }
            }
        }
    }
}
