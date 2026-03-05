use velos_core::VelosError;

use super::config::load_global_config;

/// Hidden subcommand: called by the Zig daemon via fork+exec on process crash.
/// Reads Telegram config from ~/.velos/config.toml and sends a notification.
pub async fn run(process_name: String, exit_code: i32) -> Result<(), VelosError> {
    let config = load_global_config()?;

    let telegram = match config.notifications.and_then(|n| n.telegram) {
        Some(t) if !t.bot_token.is_empty() && !t.chat_id.is_empty() => t,
        _ => return Ok(()), // No Telegram config — silently skip
    };

    // Fetch last 10 log lines for context
    let log_tail = fetch_recent_logs(&process_name).await;

    let hostname = hostname();
    let timestamp = chrono_now();

    let mut text = format!(
        "\u{1F6A8} *Process Crashed*\n\n\
         *Name:* `{process_name}`\n\
         *Exit code:* `{exit_code}`\n\
         *Host:* `{hostname}`\n\
         *Time:* {timestamp}"
    );

    if !log_tail.is_empty() {
        text.push_str("\n\n*Last logs:*\n```\n");
        text.push_str(&log_tail);
        text.push_str("\n```");
    }

    send_telegram(&telegram.bot_token, &telegram.chat_id, &text)?;

    Ok(())
}

fn send_telegram(bot_token: &str, chat_id: &str, text: &str) -> Result<(), VelosError> {
    let url = format!(
        "https://api.telegram.org/bot{bot_token}/sendMessage"
    );

    let resp = ureq::post(&url)
        .send_json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        }))
        .map_err(|e| VelosError::ProtocolError(format!("Telegram API error: {e}")))?;

    if resp.status() != 200 {
        let body = resp.into_body().read_to_string()
            .unwrap_or_default();
        return Err(VelosError::ProtocolError(format!(
            "Telegram API returned {}: {body}",
            200 // we already checked
        )));
    }

    Ok(())
}

async fn fetch_recent_logs(process_name: &str) -> String {
    let result = async {
        let mut client = crate::commands::connect().await?;
        let id = crate::commands::resolve_id(&mut client, process_name).await?;
        let logs = client.logs(id, 10).await?;
        let lines: Vec<String> = logs
            .iter()
            .map(|e| {
                let stream = if e.stream == 1 { "ERR" } else { "OUT" };
                format!("[{stream}] {}", e.message)
            })
            .collect();
        Ok::<String, VelosError>(lines.join("\n"))
    }
    .await;

    result.unwrap_or_default()
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

fn chrono_now() -> String {
    // Use simple unix timestamp formatting without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Format as human-readable UTC
    let secs_in_day = 86400u64;
    let secs_in_hour = 3600u64;
    let secs_in_min = 60u64;

    let days_since_epoch = now / secs_in_day;
    let time_of_day = now % secs_in_day;

    let hours = time_of_day / secs_in_hour;
    let minutes = (time_of_day % secs_in_hour) / secs_in_min;
    let seconds = time_of_day % secs_in_min;

    // Simple date calculation (good enough for display)
    let (year, month, day) = days_to_ymd(days_since_epoch);

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
