use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use velos_client::VelosClient;

pub fn router() -> Router {
    let (tx, _) = broadcast::channel::<String>(256);
    let tx = Arc::new(tx);

    // Spawn background poller that broadcasts process updates
    let tx_poller = tx.clone();
    tokio::spawn(async move {
        poll_daemon(tx_poller).await;
    });

    Router::new().route("/ws", get(move |ws| ws_handler(ws, tx.clone())))
}

async fn ws_handler(ws: WebSocketUpgrade, tx: Arc<broadcast::Sender<String>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

async fn handle_socket(mut socket: WebSocket, tx: Arc<broadcast::Sender<String>>) {
    let mut rx = tx.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

async fn poll_daemon(tx: Arc<broadcast::Sender<String>>) {
    let mut tick = interval(Duration::from_secs(2));
    loop {
        tick.tick().await;

        // Skip if nobody is listening
        if tx.receiver_count() == 0 {
            continue;
        }

        let Ok(mut client) = VelosClient::connect().await else {
            continue;
        };

        if let Ok(procs) = client.list().await {
            for p in &procs {
                let msg = serde_json::json!({
                    "type": "process_update",
                    "data": {
                        "name": p.name,
                        "id": p.id,
                        "pid": p.pid,
                        "status": p.status,
                        "status_str": p.status_str(),
                        "memory": p.memory_bytes,
                        "uptime_ms": p.uptime_ms,
                        "restarts": p.restart_count,
                    }
                });
                let _ = tx.send(msg.to_string());
            }
        }
    }
}
