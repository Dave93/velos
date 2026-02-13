use velos_core::VelosError;

pub async fn run(name_or_id: Option<String>, json: bool) -> Result<(), VelosError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let log_dir = std::path::PathBuf::from(&home).join(".velos").join("logs");

    if let Some(ref target) = name_or_id {
        // Flush logs for a specific process — resolve name to find log files
        let mut client = super::connect().await?;
        let id = super::resolve_id(&mut client, target).await?;
        let procs = client.list().await?;
        if let Some(p) = procs.iter().find(|p| p.id == id) {
            let out_log = log_dir.join(format!("{}-out.log", p.name));
            let err_log = log_dir.join(format!("{}-err.log", p.name));
            let _ = std::fs::write(&out_log, b"");
            let _ = std::fs::write(&err_log, b"");
            if json {
                println!("{}", serde_json::json!({ "flushed": p.name }));
            } else {
                println!("[velos] Flushed logs for '{}'", p.name);
            }
        } else {
            return Err(VelosError::ProcessNotFound(target.clone()));
        }
    } else {
        // Flush all logs
        if log_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&log_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("log") {
                        let _ = std::fs::write(entry.path(), b"");
                    }
                }
            }
        }
        if json {
            println!("{}", serde_json::json!({ "flushed": "all" }));
        } else {
            println!("[velos] Flushed all log files");
        }
    }

    Ok(())
}
