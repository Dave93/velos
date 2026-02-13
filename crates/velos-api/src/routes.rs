use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use velos_client::VelosClient;
use velos_core::protocol::StartPayload;

pub fn router() -> Router {
    Router::new()
        .route("/api/processes", get(list_processes))
        .route("/api/processes", post(start_process))
        .route("/api/processes/{name}", get(get_process))
        .route("/api/processes/{name}", delete(delete_process))
        .route("/api/processes/{name}/restart", post(restart_process))
        .route("/api/logs/{name}", get(get_logs))
}

async fn connect() -> Result<VelosClient, (StatusCode, Json<serde_json::Value>)> {
    VelosClient::connect().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("daemon unavailable: {e}")})),
        )
    })
}

fn daemon_err(e: velos_core::VelosError) -> (StatusCode, Json<serde_json::Value>) {
    let msg = e.to_string();
    if msg.contains("not found") {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": msg})),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": msg})),
        )
    }
}

async fn resolve_name(
    client: &mut VelosClient,
    name: &str,
) -> Result<u32, (StatusCode, Json<serde_json::Value>)> {
    if let Ok(id) = name.parse::<u32>() {
        return Ok(id);
    }
    let procs = client.list().await.map_err(daemon_err)?;
    procs
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("process not found: {name}")})),
            )
        })
}

// GET /api/processes
async fn list_processes() -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;
    let procs = client.list().await.map_err(daemon_err)?;
    Ok(Json(procs))
}

// GET /api/processes/:name
async fn get_process(
    Path(name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;
    let id = resolve_name(&mut client, &name).await?;
    let detail = client.info(id).await.map_err(daemon_err)?;
    Ok(Json(detail))
}

#[derive(Deserialize)]
struct StartRequest {
    name: String,
    script: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    interpreter: Option<String>,
    #[serde(default = "default_kill_timeout")]
    kill_timeout_ms: u32,
    #[serde(default = "default_true")]
    autorestart: bool,
    #[serde(default = "default_max_restarts")]
    max_restarts: i32,
    #[serde(default = "default_min_uptime")]
    min_uptime_ms: u64,
    #[serde(default)]
    restart_delay_ms: u32,
    #[serde(default)]
    exp_backoff: bool,
    #[serde(default)]
    max_memory_restart: u64,
    #[serde(default)]
    watch: bool,
    #[serde(default = "default_watch_delay")]
    watch_delay_ms: u32,
    #[serde(default)]
    watch_paths: Option<String>,
    #[serde(default)]
    watch_ignore: Option<String>,
    #[serde(default)]
    cron_restart: Option<String>,
    #[serde(default)]
    wait_ready: bool,
    #[serde(default = "default_listen_timeout")]
    listen_timeout_ms: u32,
    #[serde(default)]
    shutdown_with_message: bool,
    #[serde(default)]
    instances: Option<u32>,
}

fn default_true() -> bool {
    true
}
fn default_kill_timeout() -> u32 {
    5000
}
fn default_max_restarts() -> i32 {
    15
}
fn default_min_uptime() -> u64 {
    1000
}
fn default_watch_delay() -> u32 {
    1000
}
fn default_listen_timeout() -> u32 {
    8000
}

// POST /api/processes
async fn start_process(
    Json(body): Json<StartRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;

    let payload = StartPayload {
        name: body.name.clone(),
        script: body.script,
        cwd: body.cwd.unwrap_or_else(|| ".".into()),
        interpreter: body.interpreter,
        kill_timeout_ms: body.kill_timeout_ms,
        autorestart: body.autorestart,
        max_restarts: body.max_restarts,
        min_uptime_ms: body.min_uptime_ms,
        restart_delay_ms: body.restart_delay_ms,
        exp_backoff: body.exp_backoff,
        max_memory_restart: body.max_memory_restart,
        watch: body.watch,
        watch_delay_ms: body.watch_delay_ms,
        watch_paths: body.watch_paths.unwrap_or_default(),
        watch_ignore: body.watch_ignore.unwrap_or_default(),
        cron_restart: body.cron_restart.unwrap_or_default(),
        wait_ready: body.wait_ready,
        listen_timeout_ms: body.listen_timeout_ms,
        shutdown_with_message: body.shutdown_with_message,
        instances: body.instances.unwrap_or(1),
    };

    let result = client.start(payload).await.map_err(daemon_err)?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"id": result.id, "name": body.name})),
    ))
}

// DELETE /api/processes/:name
async fn delete_process(
    Path(name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;
    let id = resolve_name(&mut client, &name).await?;
    client.stop(id).await.map_err(daemon_err)?;
    client.delete(id).await.map_err(daemon_err)?;
    Ok(Json(serde_json::json!({"status": "deleted", "name": name})))
}

// POST /api/processes/:name/restart
async fn restart_process(
    Path(name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;
    let id = resolve_name(&mut client, &name).await?;
    client.restart(id).await.map_err(daemon_err)?;
    Ok(Json(
        serde_json::json!({"status": "restarted", "name": name}),
    ))
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default = "default_log_lines")]
    lines: u32,
    #[serde(default)]
    level: Option<String>,
}

fn default_log_lines() -> u32 {
    100
}

// GET /api/logs/:name?lines=100&level=error
async fn get_logs(
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut client = connect().await?;
    let id = resolve_name(&mut client, &name).await?;
    let entries = client.logs(id, query.lines).await.map_err(daemon_err)?;

    let filtered = if let Some(ref level) = query.level {
        let level_num = match level.as_str() {
            "error" => 3u8,
            "warn" => 2u8,
            "info" => 1u8,
            "debug" => 0u8,
            _ => 255,
        };
        entries
            .into_iter()
            .filter(|e| e.level == level_num)
            .collect()
    } else {
        entries
    };

    Ok(Json(filtered))
}
