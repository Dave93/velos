use serde_json::Value;
use velos_core::protocol::StartPayload;
use velos_core::VelosError;

/// Execute an MCP tool by name.
pub async fn execute(tool_name: &str, arguments: Value) -> Result<String, VelosError> {
    match tool_name {
        "process_list" => process_list().await,
        "process_start" => process_start(arguments).await,
        "process_stop" => process_stop(arguments).await,
        "process_restart" => process_restart(arguments).await,
        "process_delete" => process_delete(arguments).await,
        "process_info" => process_info(arguments).await,
        "log_read" => log_read(arguments).await,
        "log_search" => log_search(arguments).await,
        "log_summary" => log_summary(arguments).await,
        "health_check" => health_check().await,
        "metrics_snapshot" => metrics_snapshot(arguments).await,
        "config_get" => config_get(arguments).await,
        "config_set" => config_set(arguments).await,
        _ => Err(VelosError::ProtocolError(format!(
            "unknown tool: {tool_name}"
        ))),
    }
}

// --- Helpers ---

fn get_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_u32(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
}

async fn connect() -> Result<velos_client::VelosClient, VelosError> {
    velos_client::VelosClient::connect().await
}

async fn resolve_id(
    client: &mut velos_client::VelosClient,
    name_or_id: &str,
) -> Result<u32, VelosError> {
    if let Ok(id) = name_or_id.parse::<u32>() {
        return Ok(id);
    }
    let procs = client.list().await?;
    procs
        .iter()
        .find(|p| p.name == name_or_id)
        .map(|p| p.id)
        .ok_or_else(|| VelosError::ProcessNotFound(name_or_id.to_string()))
}

// --- Process tools ---

async fn process_list() -> Result<String, VelosError> {
    let mut client = connect().await?;
    let procs = client.list().await?;
    let compact: Vec<Value> = procs
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "status": p.status_str(),
                "pid": p.pid,
                "memory": p.memory_bytes,
                "uptime_ms": p.uptime_ms,
                "restarts": p.restart_count,
            })
        })
        .collect();
    serde_json::to_string(&compact).map_err(|e| VelosError::ProtocolError(e.to_string()))
}

async fn process_start(args: Value) -> Result<String, VelosError> {
    let script = get_string(&args, "script")
        .ok_or_else(|| VelosError::ProtocolError("missing 'script' argument".into()))?;
    let name = get_string(&args, "name").unwrap_or_else(|| {
        std::path::Path::new(&script)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app")
            .to_string()
    });
    let cwd = get_string(&args, "cwd").unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let interpreter = get_string(&args, "interpreter");

    let payload = StartPayload {
        name: name.clone(),
        script,
        cwd,
        interpreter,
        kill_timeout_ms: 5000,
        autorestart: true,
        max_restarts: 15,
        min_uptime_ms: 1000,
        restart_delay_ms: 100,
        exp_backoff: false,
        max_memory_restart: 0,
        watch: false,
        watch_delay_ms: 0,
        watch_paths: String::new(),
        watch_ignore: String::new(),
        cron_restart: String::new(),
        wait_ready: false,
        listen_timeout_ms: 8000,
        shutdown_with_message: false,
        instances: 1,
    };

    let mut client = connect().await?;
    let result = client.start(payload).await?;
    Ok(serde_json::json!({
        "id": result.id,
        "name": name,
        "status": "running"
    })
    .to_string())
}

async fn process_stop(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    client.stop(id).await?;
    Ok(
        serde_json::json!({"success": true, "message": format!("stopped {name_or_id}")})
            .to_string(),
    )
}

async fn process_restart(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    client.restart(id).await?;
    Ok(
        serde_json::json!({"success": true, "message": format!("restarted {name_or_id}")})
            .to_string(),
    )
}

async fn process_delete(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    client.delete(id).await?;
    Ok(
        serde_json::json!({"success": true, "message": format!("deleted {name_or_id}")})
            .to_string(),
    )
}

async fn process_info(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    let info = client.info(id).await?;
    Ok(serde_json::json!({
        "id": info.id,
        "name": info.name,
        "pid": info.pid,
        "status": info.status_str(),
        "memory": info.memory_bytes,
        "uptime_ms": info.uptime_ms,
        "restarts": info.restart_count,
        "script": info.script,
        "cwd": info.cwd,
        "interpreter": info.interpreter,
        "autorestart": info.autorestart,
        "max_restarts": info.max_restarts,
    })
    .to_string())
}

// --- Log tools ---

async fn log_read(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let lines = get_u32(&args, "lines").unwrap_or(50);
    let level_filter = get_string(&args, "level");

    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    let entries = client.logs(id, lines).await?;

    let classifier = velos_log_engine::classifier::Classifier::with_defaults();
    let mut processed = classifier.classify_batch(&entries);

    if let Some(ref levels) = level_filter {
        let allowed = parse_levels(levels);
        processed.retain(|e| allowed.contains(&e.level));
    }

    let compact: Vec<Value> = processed
        .iter()
        .map(|e| {
            serde_json::json!({
                "t": e.timestamp_ms,
                "l": e.level.as_str(),
                "m": e.message,
            })
        })
        .collect();
    serde_json::to_string(&compact).map_err(|e| VelosError::ProtocolError(e.to_string()))
}

async fn log_search(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let pattern = get_string(&args, "pattern")
        .ok_or_else(|| VelosError::ProtocolError("missing 'pattern'".into()))?;
    let level_filter = get_string(&args, "level");

    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    let entries = client.logs(id, 500).await?;

    let classifier = velos_log_engine::classifier::Classifier::with_defaults();
    let mut processed = classifier.classify_batch(&entries);

    if let Some(ref levels) = level_filter {
        let allowed = parse_levels(levels);
        processed.retain(|e| allowed.contains(&e.level));
    }

    let re = regex::Regex::new(&pattern)
        .map_err(|e| VelosError::ProtocolError(format!("invalid regex pattern: {e}")))?;
    processed.retain(|e| re.is_match(&e.message));

    let compact: Vec<Value> = processed
        .iter()
        .map(|e| {
            serde_json::json!({
                "t": e.timestamp_ms,
                "l": e.level.as_str(),
                "m": e.message,
            })
        })
        .collect();
    serde_json::to_string(&compact).map_err(|e| VelosError::ProtocolError(e.to_string()))
}

async fn log_summary(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let lines = get_u32(&args, "lines").unwrap_or(200);

    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    let entries = client.logs(id, lines).await?;

    let classifier = velos_log_engine::classifier::Classifier::with_defaults();
    let processed = classifier.classify_batch(&entries);

    let detector = velos_log_engine::pattern::PatternDetector::with_defaults();
    let patterns = detector.detect(&processed);

    let summary =
        velos_log_engine::summary::generate_summary(&name_or_id, &processed, &patterns, &[], 0);

    serde_json::to_string_pretty(&summary).map_err(|e| VelosError::ProtocolError(e.to_string()))
}

// --- Monitoring tools ---

async fn health_check() -> Result<String, VelosError> {
    let mut client = connect().await?;
    let procs = client.list().await?;

    let mut overall_score = 100i32;
    let mut process_health: Vec<Value> = Vec::new();

    for p in &procs {
        let mut score = 100i32;
        let mut issues: Vec<String> = Vec::new();

        if p.status_str() != "running" {
            score -= 50;
            issues.push(format!("status: {}", p.status_str()));
        }
        if p.restart_count > 0 {
            score -= (p.restart_count as i32 * 3).min(30);
            issues.push(format!("{} restarts", p.restart_count));
        }
        score = score.max(0);

        process_health.push(serde_json::json!({
            "name": p.name,
            "score": score,
            "status": p.status_str(),
            "issues": issues,
        }));

        overall_score = overall_score.min(score);
    }

    Ok(serde_json::json!({
        "overall_score": overall_score,
        "process_count": procs.len(),
        "processes": process_health,
    })
    .to_string())
}

async fn metrics_snapshot(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id");
    let mut client = connect().await?;

    if let Some(ref nid) = name_or_id {
        let id = resolve_id(&mut client, nid).await?;
        let info = client.info(id).await?;
        Ok(serde_json::json!({
            "name": info.name,
            "memory": info.memory_bytes,
            "uptime_ms": info.uptime_ms,
            "restarts": info.restart_count,
            "status": info.status_str(),
        })
        .to_string())
    } else {
        let procs = client.list().await?;
        let metrics: Vec<Value> = procs
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "memory": p.memory_bytes,
                    "uptime_ms": p.uptime_ms,
                    "restarts": p.restart_count,
                    "status": p.status_str(),
                })
            })
            .collect();
        serde_json::to_string(&metrics).map_err(|e| VelosError::ProtocolError(e.to_string()))
    }
}

async fn config_get(args: Value) -> Result<String, VelosError> {
    let name_or_id = get_string(&args, "name_or_id")
        .ok_or_else(|| VelosError::ProtocolError("missing 'name_or_id'".into()))?;
    let mut client = connect().await?;
    let id = resolve_id(&mut client, &name_or_id).await?;
    let info = client.info(id).await?;
    Ok(serde_json::json!({
        "name": info.name,
        "script": info.script,
        "cwd": info.cwd,
        "interpreter": info.interpreter,
        "kill_timeout_ms": info.kill_timeout_ms,
        "autorestart": info.autorestart,
        "max_restarts": info.max_restarts,
        "min_uptime_ms": info.min_uptime_ms,
        "restart_delay_ms": info.restart_delay_ms,
        "exp_backoff": info.exp_backoff,
        "max_memory_restart": info.max_memory_restart,
        "watch": info.watch,
        "cron_restart": info.cron_restart,
        "wait_ready": info.wait_ready,
        "shutdown_with_message": info.shutdown_with_message,
    })
    .to_string())
}

async fn config_set(_args: Value) -> Result<String, VelosError> {
    Err(VelosError::ProtocolError(
        "config_set not yet implemented (requires daemon support)".into(),
    ))
}

// --- Utility ---

fn parse_levels(levels_str: &str) -> Vec<velos_log_engine::LogLevel> {
    levels_str
        .split(',')
        .filter_map(|l| match l.trim().to_lowercase().as_str() {
            "debug" => Some(velos_log_engine::LogLevel::Debug),
            "info" => Some(velos_log_engine::LogLevel::Info),
            "warn" | "warning" => Some(velos_log_engine::LogLevel::Warn),
            "error" | "err" => Some(velos_log_engine::LogLevel::Error),
            "fatal" => Some(velos_log_engine::LogLevel::Fatal),
            _ => None,
        })
        .collect()
}
