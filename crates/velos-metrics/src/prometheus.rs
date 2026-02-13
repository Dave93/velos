use std::fmt::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, response::IntoResponse, routing::get, Router};
use tokio::sync::RwLock;
use velos_client::VelosClient;
use velos_core::protocol::ProcessInfo;

/// Cached process list, refreshed periodically.
struct MetricsState {
    processes: Vec<ProcessInfo>,
}

/// Start the Prometheus metrics HTTP server.
///
/// Connects to the daemon and exposes `/metrics` in Prometheus text format.
/// `poll_interval` controls how frequently the daemon is queried.
pub async fn serve(port: u16, poll_interval: Duration) -> Result<(), velos_core::VelosError> {
    let state = Arc::new(RwLock::new(MetricsState {
        processes: Vec::new(),
    }));

    // Background poller
    let poller_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match VelosClient::connect().await {
                Ok(mut client) => match client.list().await {
                    Ok(procs) => {
                        poller_state.write().await.processes = procs;
                    }
                    Err(e) => {
                        eprintln!("[velos-metrics] poll error: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("[velos-metrics] connect error: {e}");
                }
            }
            tokio::time::sleep(poll_interval).await;
        }
    });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Prometheus metrics server listening on http://{addr}/metrics");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| velos_core::VelosError::ProtocolError(format!("bind error: {e}")))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| velos_core::VelosError::ProtocolError(format!("server error: {e}")))
}

async fn metrics_handler(State(state): State<Arc<RwLock<MetricsState>>>) -> impl IntoResponse {
    let snap = state.read().await;
    let body = format_metrics(&snap.processes);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

fn format_metrics(processes: &[ProcessInfo]) -> String {
    let mut out = String::with_capacity(4096);

    // --- per-process metrics ---

    write_help_type(
        &mut out,
        "velos_process_cpu_percent",
        "CPU usage percentage",
        "gauge",
    );
    for p in processes {
        // CPU is not available from the list endpoint; emit 0 as placeholder.
        // A future MetricsGet IPC command can provide real CPU data.
        writeln!(
            out,
            "velos_process_cpu_percent{{name=\"{}\",instance=\"{}\"}} 0",
            escape(&p.name),
            p.id
        )
        .ok();
    }

    write_help_type(
        &mut out,
        "velos_process_memory_bytes",
        "Resident memory in bytes",
        "gauge",
    );
    for p in processes {
        writeln!(
            out,
            "velos_process_memory_bytes{{name=\"{}\",instance=\"{}\"}} {}",
            escape(&p.name),
            p.id,
            p.memory_bytes
        )
        .ok();
    }

    write_help_type(
        &mut out,
        "velos_process_uptime_seconds",
        "Process uptime in seconds",
        "gauge",
    );
    for p in processes {
        let secs = p.uptime_ms as f64 / 1000.0;
        writeln!(
            out,
            "velos_process_uptime_seconds{{name=\"{}\",instance=\"{}\"}} {:.3}",
            escape(&p.name),
            p.id,
            secs
        )
        .ok();
    }

    write_help_type(
        &mut out,
        "velos_process_restart_total",
        "Total restart count",
        "counter",
    );
    for p in processes {
        writeln!(
            out,
            "velos_process_restart_total{{name=\"{}\",instance=\"{}\"}} {}",
            escape(&p.name),
            p.id,
            p.restart_count
        )
        .ok();
    }

    write_help_type(
        &mut out,
        "velos_process_status",
        "Process status (0=stopped, 1=online, 2=errored)",
        "gauge",
    );
    for p in processes {
        let status_val = match p.status {
            0 => 0, // stopped
            1 => 1, // running → online
            2 => 2, // errored
            3 => 1, // starting → treat as online
            _ => 0,
        };
        writeln!(
            out,
            "velos_process_status{{name=\"{}\",instance=\"{}\"}} {}",
            escape(&p.name),
            p.id,
            status_val
        )
        .ok();
    }

    // --- daemon-level metrics ---

    write_help_type(
        &mut out,
        "velos_daemon_processes_total",
        "Number of managed processes",
        "gauge",
    );
    writeln!(out, "velos_daemon_processes_total {}", processes.len()).ok();

    out
}

fn write_help_type(out: &mut String, name: &str, help: &str, metric_type: &str) {
    writeln!(out, "# HELP {name} {help}").ok();
    writeln!(out, "# TYPE {name} {metric_type}").ok();
}

/// Escape label values for Prometheus text format.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_metrics_empty() {
        let out = format_metrics(&[]);
        assert!(out.contains("velos_daemon_processes_total 0"));
    }

    #[test]
    fn test_format_metrics_one_process() {
        let procs = vec![ProcessInfo {
            id: 0,
            name: "api".to_string(),
            pid: 1234,
            status: 1,
            memory_bytes: 47_185_920,
            uptime_ms: 86_400_000,
            restart_count: 3,
        }];
        let out = format_metrics(&procs);
        assert!(out.contains("velos_process_memory_bytes{name=\"api\",instance=\"0\"} 47185920"));
        assert!(out.contains("velos_process_uptime_seconds{name=\"api\",instance=\"0\"} 86400.000"));
        assert!(out.contains("velos_process_restart_total{name=\"api\",instance=\"0\"} 3"));
        assert!(out.contains("velos_process_status{name=\"api\",instance=\"0\"} 1"));
        assert!(out.contains("velos_daemon_processes_total 1"));
    }

    #[test]
    fn test_escape_label() {
        assert_eq!(escape("hello\"world"), "hello\\\"world");
        assert_eq!(escape("line\nbreak"), "line\\nbreak");
    }
}
