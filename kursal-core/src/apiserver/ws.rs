use super::{APIAppState, types::APIEvent};
use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

#[utoipa::path(
    get,
    path = "/ws",
    tag = "Events",
    description = "Upgrades to a WebSocket streaming core events as JSON text frames. Frames sent by the client are ignored; close the socket to unsubscribe.",
    responses(
        (status = 101, description = "Switching protocols, then one frame per event", body = APIEvent)
    )
)]
pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<APIAppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: APIAppState) {
    let mut event_rx = state.event_tx.subscribe();
    let (mut sink, mut stream) = socket.split();

    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let Ok(msg) = serde_json::to_string(&APIEvent {
                        event: event.event,
                        payload: event.payload,
                    }) else {
                        continue;
                    };

                    if sink.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Websocket client lagged, dropped {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }

    send_task.abort();
}
