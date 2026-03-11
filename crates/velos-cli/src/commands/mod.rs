// AI output key abbreviations (--ai flag):
// n = name, i = id, s = status, p = pid
// m = memory (bytes), u = uptime (ms), r = restarts
// c = cpu (percent), t = timestamp (ms), l = level

pub mod ai;
pub mod api;
pub mod completions;
pub mod config;
pub mod daemon;
pub mod delete;
pub mod flush;
pub mod info;
pub mod list;
pub mod logs;
pub mod metrics;
pub mod monit;
pub mod notify_crash;
pub mod notify_error;
pub mod ping;
pub mod reload;
pub mod restart;
pub mod resurrect;
pub mod save;
pub mod scale;
pub mod start;
pub mod startup;
pub mod stop;
pub mod telegram_poller;

use velos_client::VelosClient;
use velos_core::VelosError;

/// Helper: connect to the daemon, auto-starting it if not running.
pub async fn connect() -> Result<VelosClient, VelosError> {
    match VelosClient::connect().await {
        Ok(client) => Ok(client),
        Err(_) if !velos_client::is_daemon_running() => {
            // Auto-start daemon in background
            ensure_daemon_running()?;
            // Wait for socket to appear (up to 5s)
            let socket = velos_client::default_socket_path();
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if socket.exists() {
                    if let Ok(client) = VelosClient::connect().await {
                        return Ok(client);
                    }
                }
            }
            Err(VelosError::ProtocolError(
                "Daemon started but could not connect. Check: velos daemon".into(),
            ))
        }
        Err(e) => Err(e),
    }
}

/// Spawn daemon as a background process if it's not already running.
fn ensure_daemon_running() -> Result<(), VelosError> {
    let exe = std::env::current_exe().map_err(|e| VelosError::Io(e))?;

    eprintln!("[velos] Daemon not running — starting automatically...");

    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".velos")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let stdout_log =
        std::fs::File::create(log_dir.join("daemon-stdout.log")).map_err(VelosError::Io)?;
    let stderr_log =
        std::fs::File::create(log_dir.join("daemon-stderr.log")).map_err(VelosError::Io)?;

    std::process::Command::new(&exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout_log))
        .stderr(std::process::Stdio::from(stderr_log))
        .spawn()
        .map_err(|e| VelosError::Io(e))?;

    Ok(())
}

/// Resolve a name-or-ID string to a numeric process ID.
/// If it parses as u32, use it directly. Otherwise, query the process list
/// and find the first process whose name matches.
pub async fn resolve_id(client: &mut VelosClient, name_or_id: &str) -> Result<u32, VelosError> {
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

/// Resolve a name-or-ID to one or more process IDs (cluster-aware).
/// "api" matches all instances: "api", "api:0", "api:1", etc.
/// "api:2" matches only that specific instance.
/// A numeric string matches exactly one process by ID.
pub async fn resolve_ids(
    client: &mut VelosClient,
    name_or_id: &str,
) -> Result<Vec<u32>, VelosError> {
    if let Ok(id) = name_or_id.parse::<u32>() {
        return Ok(vec![id]);
    }

    let procs = client.list().await?;

    // First try exact match
    let exact: Vec<u32> = procs
        .iter()
        .filter(|p| p.name == name_or_id)
        .map(|p| p.id)
        .collect();
    if !exact.is_empty() {
        return Ok(exact);
    }

    // Then try as base name (match "name:N" pattern for cluster instances)
    let cluster: Vec<u32> = procs
        .iter()
        .filter(|p| {
            p.name.len() > name_or_id.len()
                && p.name.starts_with(name_or_id)
                && p.name.as_bytes().get(name_or_id.len()) == Some(&b':')
                && p.name[name_or_id.len() + 1..].parse::<u32>().is_ok()
        })
        .map(|p| p.id)
        .collect();

    if !cluster.is_empty() {
        return Ok(cluster);
    }

    Err(VelosError::ProcessNotFound(name_or_id.to_string()))
}
