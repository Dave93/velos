use velos_core::VelosError;

pub async fn run(name: String, lines: u32, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;

    let id: u32 = name
        .parse()
        .map_err(|_| VelosError::ProcessNotFound(name.clone()))?;

    let entries = client.logs(id, lines).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries).unwrap_or_default());
        return Ok(());
    }

    if entries.is_empty() {
        println!("[velos] No log entries for '{}'", name);
        return Ok(());
    }

    for entry in &entries {
        let stream_label = match entry.stream {
            0 => "out",
            1 => "err",
            _ => "???",
        };
        println!("[{}|{}] {}", stream_label, entry.timestamp_ms, entry.message);
    }

    Ok(())
}
