use crate::serve;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
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
            msg = ws.recv() => {
                match msg {
                    Some(Ok(Message::Close(reason))) => {
                        tracing::debug!("received close from browser: {reason:?}");
                        let _ = ws.send(Message::Close(reason)).await;
                        let _ = ws.close().await;
                        return
                    }
                    Some(Ok(Message::Ping(msg))) => {
                        tracing::trace!("responding to Ping");
                        let _ = ws.send(Message::Pong(msg)).await;
                    }
                    Some(Ok(msg)) => {
                        tracing::debug!("received message from browser: {msg:?} (ignoring)");
                    }
                    Some(Err(err))=> {
                        tracing::debug!("autoreload websocket closed: {err}");
                        return
                    }
                    None => {
                        tracing::debug!("lost websocket");
                        return
                    }
                }
            }
            state = rx.next() => {

                let state = match state {
                    Some(state) => state,
                    None => {
                        tracing::debug!("state watcher closed");
                        return
                    },
                };

                tracing::trace!("Build state changed: {state:?}");

                let msg = match state {
                    State::Ok if first => {
                        // If the state is ok, and it's the first message we would send, discard it,
                        // as this would cause a reload right after connecting. On the other side,
                        // we want to send out a failed build even after reconnecting.
                        first = false;
                        tracing::trace!("Discarding first reload trigger");
                        None
                    },
                    State::Ok  => Some(ClientMessage::Reload),
                    State::Failed { reason } => Some(ClientMessage::BuildFailure { reason }),
                };

                tracing::trace!("Message to send: {msg:?}");

                if let Some(msg) = msg {
                    if let Ok(text) = serde_json::to_string(&msg) {
                        if let Err(err) = ws.send(Message::Text(text.into())).await {
                            tracing::info!("autoload websocket failed to send: {err}");
                            break;
                        }
                    }
                }
            }
        }
    }

    tracing::debug!("exiting WS handler");
}
