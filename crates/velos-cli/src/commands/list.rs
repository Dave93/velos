use tabled::{
    builder::Builder,
    settings::{
        object::{Columns, Rows},
        style::Style,
        Alignment, Color, Modify,
    },
};
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
        println!(
            "{}",
            serde_json::to_string_pretty(&procs).unwrap_or_default()
        );
        return Ok(());
    }

    if procs.is_empty() {
        println!("[velos] No processes running");
        return Ok(());
    }

    let mut builder = Builder::new();
    builder.push_record(["id", "name", "pid", "mode", "status", "uptime", "restarts", "mem"]);

    for p in &procs {
        let mode = if p.name.contains(':') { "cluster" } else { "fork" };
        let status = p.status_str();
        let pid_str = if p.pid > 0 {
            p.pid.to_string()
        } else {
            "N/A".to_string()
        };
        let mem = if p.memory_bytes > 0 {
            format_bytes(p.memory_bytes)
        } else {
            "0b".to_string()
        };

        builder.push_record([
            &p.id.to_string(),
            &p.name,
            &pid_str,
            mode,
            status,
            &format_uptime(p.uptime_ms),
            &p.restart_count.to_string(),
            &mem,
        ]);
    }

    let mut table = builder.build();
    table
        .with(Style::rounded())
        .with(Modify::new(Rows::first()).with(Color::new("\x1b[1;37m", "\x1b[0m")))
        .with(Modify::new(Columns::single(0)).with(Alignment::right()));

    // Color status column (index 4) per-row
    let table_str = table.to_string();
    let mut lines: Vec<String> = table_str.lines().map(String::from).collect();

    for line in &mut lines {
        // Color statuses
        if line.contains(" online ") || line.contains(" online │") || line.contains("│ online ") {
            *line = line.replace("online", "\x1b[32monline\x1b[0m");
        } else if line.contains("running") && !line.contains("\x1b[1;37m") {
            *line = line.replace("running", "\x1b[32mrunning\x1b[0m");
        } else if line.contains("errored") {
            *line = line.replace("errored", "\x1b[31merrored\x1b[0m");
        } else if line.contains("stopped") {
            *line = line.replace("stopped", "\x1b[33mstopped\x1b[0m");
        } else if line.contains("starting") {
            *line = line.replace("starting", "\x1b[36mstarting\x1b[0m");
        }
        // Color mode
        if line.contains(" fork ") || line.contains(" fork │") || line.contains("│ fork ") {
            *line = line.replace("fork", "\x1b[36mfork\x1b[0m");
        }
        if line.contains("cluster") && !line.contains("\x1b[1;37m") {
            *line = line.replace("cluster", "\x1b[34mcluster\x1b[0m");
        }
        // Color names (column 1) — find text between first two │ delimiters after id
        // We'll color process names in the data rows
    }

    println!("{}", lines.join("\n"));
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}b")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}kb", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}mb", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}gb", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_uptime(ms: u64) -> String {
    let secs = ms / 1000;
    if secs == 0 {
        "0s".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}D", secs / 86400)
    }
}
