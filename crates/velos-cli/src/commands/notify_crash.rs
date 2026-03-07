use std::path::Path;

use velos_ai::analyzer::{
    self, CrashContext, CrashRecord, CrashStatus, SourceSnippet,
};
use velos_ai::i18n::I18n;
use velos_ai::provider::create_provider;
use velos_ai::types::AiConfig;
use velos_core::VelosError;

use super::config::load_global_config;

/// Hidden subcommand: called by the Zig daemon via fork+exec on process crash.
/// Gathers logs, runs AI analysis (if configured), saves CrashRecord,
/// and sends a Telegram notification with inline Fix/Ignore buttons.
pub async fn run(process_name: String, exit_code: i32) -> Result<(), VelosError> {
    let config = load_global_config()?;

    let language = config
        .notifications
        .as_ref()
        .and_then(|n| n.language.as_deref())
        .unwrap_or("en");
    let i18n = I18n::new(language);

    // Fetch last 20 log lines for context
    let log_lines = fetch_recent_logs(&process_name).await;

    let hostname = hostname();
    let timestamp = chrono_now();

    // Resolve working directory for source context
    let cwd = resolve_process_cwd(&process_name).await;

    // Extract source references from stack traces
    let source_refs = analyzer::extract_source_refs(&log_lines);
    let cwd_path = Path::new(&cwd);
    let source_snippets: Vec<SourceSnippet> = source_refs
        .iter()
        .filter_map(|(file, line)| analyzer::read_source_context(file, *line, cwd_path, 5))
        .collect();

    // Build crash context
    let ctx = CrashContext {
        process_name: process_name.clone(),
        exit_code,
        hostname: hostname.clone(),
        timestamp: timestamp.clone(),
        cwd: cwd.clone(),
        logs: log_lines.clone(),
        source_snippets,
    };

    // Run AI analysis if configured
    let (ai_analysis, ai_configured) = run_ai_analysis(&config.ai, &ctx);

    // Create and save crash record
    let crash_id = uuid::Uuid::new_v4().to_string();
    let record = CrashRecord {
        id: crash_id.clone(),
        process_name: process_name.clone(),
        exit_code,
        hostname: hostname.clone(),
        timestamp: timestamp.clone(),
        cwd: cwd.clone(),
        logs: log_lines.clone(),
        analysis: ai_analysis.clone().unwrap_or_default(),
        status: CrashStatus::Pending,
        fix_result: None,
        language: language.to_string(),
    };
    if let Err(e) = record.save() {
        eprintln!("[velos] failed to save crash record: {e}");
    }

    // Send Telegram notification
    let telegram = config.notifications.and_then(|n| n.telegram);
    if let Some(t) = telegram {
        if !t.bot_token.is_empty() && !t.chat_id.is_empty() {
            let text = build_telegram_message(&i18n, &process_name, exit_code, &hostname, &timestamp, &log_lines, &ai_analysis, ai_configured);
            if let Err(e) = send_telegram_with_buttons(&t.bot_token, &t.chat_id, &text, &crash_id, &i18n) {
                eprintln!("[velos] Telegram send error: {e}");
            }
        }
    }

    Ok(())
}

/// Returns (analysis_result, is_ai_configured).
fn run_ai_analysis(
    ai_config: &Option<super::config::AiConfigToml>,
    ctx: &CrashContext,
) -> (Option<String>, bool) {
    let ai = match ai_config.as_ref() {
        Some(ai) if !ai.provider.is_empty() && !ai.api_key.is_empty() => ai,
        _ => return (None, false),
    };

    if !ai.auto_analyze {
        return (None, true);
    }

    let config = AiConfig {
        provider: ai.provider.clone(),
        model: ai.model.clone(),
        api_key: ai.api_key.clone(),
        base_url: ai.base_url.clone(),
        max_iterations: ai.max_iterations,
        auto_analyze: ai.auto_analyze,
        auto_fix: ai.auto_fix,
    };

    let provider = match create_provider(&config) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[velos] AI provider error: {e}");
            return (None, true);
        }
    };

    match analyzer::analyze(provider.as_ref(), ctx) {
        Ok(analysis) => (Some(analysis), true),
        Err(e) => {
            eprintln!("[velos] AI analysis error: {e}");
            (None, true)
        }
    }
}

fn build_telegram_message(
    i18n: &I18n,
    process_name: &str,
    exit_code: i32,
    hostname: &str,
    timestamp: &str,
    log_lines: &[String],
    ai_analysis: &Option<String>,
    ai_configured: bool,
) -> String {
    let h = html_escape;
    let mut text = format!(
        "\u{1F6A8} <b>{}</b>\n\n\
         <b>{}:</b> <code>{}</code>\n\
         <b>{}:</b> <code>{}</code>\n\
         <b>{}:</b> <code>{}</code>\n\
         <b>{}:</b> {}",
        h(i18n.get("crash.title")),
        h(i18n.get("crash.name")), h(process_name),
        h(i18n.get("crash.exit_code")), exit_code,
        h(i18n.get("crash.host")), h(hostname),
        h(i18n.get("crash.time")), h(timestamp),
    );

    if !log_lines.is_empty() {
        let log_tail: String = log_lines
            .iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        text.push_str(&format!(
            "\n\n<b>{}:</b>\n<pre>{}</pre>",
            h(i18n.get("crash.logs")),
            h(&log_tail),
        ));
    }

    match ai_analysis {
        Some(analysis) => {
            text.push_str(&format!(
                "\n\n<b>{}:</b>\n{}",
                h(i18n.get("crash.analysis_header")),
                h(analysis),
            ));
        }
        None if !ai_configured => {
            text.push_str(&format!(
                "\n\n<i>{}</i>",
                h(i18n.get("crash.no_analysis")),
            ));
        }
        None => {
            // AI configured but analysis failed — don't show misleading "not configured" message
        }
    }

    text
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn send_telegram_with_buttons(
    bot_token: &str,
    chat_id: &str,
    text: &str,
    crash_id: &str,
    i18n: &I18n,
) -> Result<(), VelosError> {
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");

    let inline_keyboard = serde_json::json!({
        "inline_keyboard": [[
            {
                "text": i18n.get("crash.btn_fix"),
                "callback_data": format!("fix:{crash_id}")
            },
            {
                "text": i18n.get("crash.btn_ignore"),
                "callback_data": format!("ignore:{crash_id}")
            }
        ]]
    });

    ureq::post(&url)
        .send_json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "reply_markup": inline_keyboard,
        }))
        .map_err(|e| VelosError::ProtocolError(format!("Telegram API error: {e}")))?;

    Ok(())
}

async fn fetch_recent_logs(process_name: &str) -> Vec<String> {
    let result = async {
        let mut client = crate::commands::connect().await?;
        let id = crate::commands::resolve_id(&mut client, process_name).await?;
        let logs = client.logs(id, 20).await?;
        let lines: Vec<String> = logs
            .iter()
            .map(|e| {
                let stream = if e.stream == 1 { "ERR" } else { "OUT" };
                format!("[{stream}] {}", e.message)
            })
            .collect();
        Ok::<Vec<String>, VelosError>(lines)
    }
    .await;

    result.unwrap_or_default()
}

async fn resolve_process_cwd(process_name: &str) -> String {
    let result = async {
        let mut client = crate::commands::connect().await?;
        let id = crate::commands::resolve_id(&mut client, process_name).await?;
        let info = client.info(id).await?;
        Ok::<String, VelosError>(info.cwd.clone())
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let secs_in_day = 86400u64;
    let secs_in_hour = 3600u64;
    let secs_in_min = 60u64;

    let days_since_epoch = now / secs_in_day;
    let time_of_day = now % secs_in_day;

    let hours = time_of_day / secs_in_hour;
    let minutes = (time_of_day % secs_in_hour) / secs_in_min;
    let seconds = time_of_day % secs_in_min;

    let (year, month, day) = days_to_ymd(days_since_epoch);

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
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
