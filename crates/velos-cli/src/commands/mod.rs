// AI output key abbreviations (--ai flag):
// n = name, i = id, s = status, p = pid
// m = memory (bytes), u = uptime (ms), r = restarts
// c = cpu (percent), t = timestamp (ms), l = level

pub mod api;
pub mod completions;
pub mod daemon;
pub mod delete;
pub mod flush;
pub mod info;
pub mod list;
pub mod logs;
pub mod metrics;
pub mod monit;
pub mod ping;
pub mod reload;
pub mod restart;
pub mod resurrect;
pub mod save;
pub mod scale;
pub mod start;
pub mod startup;
pub mod stop;

use velos_client::VelosClient;
use velos_core::VelosError;

/// Helper: connect to the daemon, printing a helpful message if not running.
pub async fn connect() -> Result<VelosClient, VelosError> {
    VelosClient::connect().await
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
