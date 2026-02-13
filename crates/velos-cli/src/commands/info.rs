use velos_core::VelosError;

pub async fn run(name_or_id: String, json: bool, ai: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let id = super::resolve_id(&mut client, &name_or_id).await?;
    let detail = client.info(id).await?;

    if ai {
        let compact = serde_json::json!({
            "n": detail.name,
            "s": detail.status_str(),
            "p": detail.pid,
            "m": detail.memory_bytes,
            "u": detail.uptime_ms,
            "r": detail.restart_count,
            "script": detail.script,
            "cwd": detail.cwd,
        });
        println!("{}", serde_json::to_string(&compact).unwrap_or_default());
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&detail).unwrap_or_default());
        return Ok(());
    }

    println!("  Name:           {}", detail.name);
    println!("  ID:             {}", detail.id);
    println!("  Script:         {}", detail.script);
    println!("  CWD:            {}", detail.cwd);
    println!("  PID:            {}", detail.pid);
    println!("  Status:         {}", detail.status_str());
    println!("  Memory:         {}", format_bytes(detail.memory_bytes));
    println!("  Uptime:         {}", format_uptime(detail.uptime_ms));
    println!("  Restarts:       {}", detail.restart_count);
    if detail.consecutive_crashes > 0 {
        println!("  Crashes:        {}", detail.consecutive_crashes);
    }
    if !detail.interpreter.is_empty() {
        println!("  Interpreter:    {}", detail.interpreter);
    }
    println!("  Autorestart:    {}", detail.autorestart);
    println!("  Max restarts:   {}", if detail.max_restarts < 0 { "unlimited".to_string() } else { detail.max_restarts.to_string() });
    println!("  Kill timeout:   {} ms", detail.kill_timeout_ms);
    if detail.exp_backoff {
        println!("  Exp backoff:    true (delay: {} ms)", detail.restart_delay_ms);
    }
    if detail.max_memory_restart > 0 {
        println!("  Max memory:     {}", format_bytes(detail.max_memory_restart));
    }
    if detail.watch {
        println!("  Watch mode:     enabled");
    }
    if !detail.cron_restart.is_empty() {
        println!("  Cron restart:   {}", detail.cron_restart);
    }
    if detail.wait_ready {
        println!("  Wait ready:     true");
    }
    if detail.shutdown_with_message {
        println!("  Shutdown msg:   true");
    }

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
