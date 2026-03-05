use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::server::McpServer;

struct AppState {
    server: McpServer,
}

/// Start the MCP server with Streamable HTTP transport.
pub async fn run_http(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState {
        server: McpServer::new(),
    });

    let app = Router::new()
        .route("/mcp", post(handle_post))
        .route("/mcp", get(handle_sse))
        .route("/health", get(handle_health))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("[velos] MCP server (Streamable HTTP) listening on http://{addr}/mcp");
    eprintln!("[velos] Health check: http://{addr}/health");
    eprintln!();
    eprintln!("[velos] Configure your AI client with:");
    eprintln!("  URL: http://<your-host>:{port}/mcp");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// POST /mcp — receive JSON-RPC request, return JSON-RPC response.
/// If Accept: text/event-stream, respond with SSE stream.
async fn handle_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::response::IntoResponse;

    let method = body.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = body.get("id").cloned();
    let params = body.get("params").cloned();

    // Notifications (no id) — accept but no response needed
    if id.is_none() {
        return Ok(StatusCode::ACCEPTED.into_response());
    }

    let request_id = id.unwrap_or(Value::Null);

    let result = state.server.handle_method(method, params).await;

    let response = match result {
        Ok(value) => serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": value
        }),
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": e.code,
                "message": e.message
            }
        }),
    };

    // Check if client wants SSE
    let accept = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if accept.contains("text/event-stream") {
        // Return as SSE stream with single event then close
        let (tx, rx) = mpsc::channel::<Value>(1);
        tx.send(response).await.ok();
        drop(tx);

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|msg| {
            Ok::<_, std::convert::Infallible>(
                Event::default()
                    .event("message")
                    .data(serde_json::to_string(&msg).unwrap_or_default()),
            )
        });

        Ok(Sse::new(stream).into_response())
    } else {
        Ok(Json(response).into_response())
    }
}

/// GET /mcp — SSE endpoint for server-initiated messages (keep-alive).
async fn handle_sse(
    State(_state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (_tx, rx) = mpsc::channel::<Value>(32);

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|msg| {
        Ok::<_, std::convert::Infallible>(
            Event::default()
                .event("message")
                .data(serde_json::to_string(&msg).unwrap_or_default()),
        )
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// GET /health — simple health check
async fn handle_health() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ok",
        "server": "velos-mcp",
        "transport": "streamable-http"
    }))
}

use futures::StreamExt;
