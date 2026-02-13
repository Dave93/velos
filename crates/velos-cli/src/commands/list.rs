use comfy_table::{Cell, Table};
use velos_core::VelosError;

pub async fn run(json: bool, ai: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let procs = client.list().await?;

    if ai {
        let compact: Vec<_> = procs
            .iter()
            .map(|p| {
                serde_json::json!({
                    "n": p.name,
                    "i": p.id,
                    "s": p.status_str(),
                    "m": p.memory_bytes,
                    "u": p.uptime_ms,
                    "r": p.restart_count,
                    "p": p.pid,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&compact).unwrap_or_default());
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&procs).unwrap_or_default());
        return Ok(());
    }

    if procs.is_empty() {
        println!("[velos] No processes running");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "PID", "Status", "Memory", "Uptime", "Restarts"]);

    for p in &procs {
        table.add_row(vec![
            Cell::new(p.id),
            Cell::new(&p.name),
            Cell::new(p.pid),
            Cell::new(p.status_str()),
            Cell::new(format_bytes(p.memory_bytes)),
            Cell::new(format_uptime(p.uptime_ms)),
            Cell::new(p.restart_count),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_uptime(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
