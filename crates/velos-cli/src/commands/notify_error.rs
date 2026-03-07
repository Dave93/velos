use velos_ai::analyzer::{self, CrashContext, CrashRecord, CrashStatus, SourceSnippet};
use velos_ai::i18n::I18n;
use velos_ai::provider::create_provider;
use velos_ai::types::AiConfig;
use velos_core::VelosError;

use super::config::load_global_config;

/// Hidden subcommand: called by the Zig daemon when an error pattern is detected
/// in stderr logs of a running process (like Sentry — no crash needed).
pub async fn run(process_name: String) -> Result<(), VelosError> {
    // Skip notification if suppressed (e.g. after AI fix restart)
    if super::ai::take_suppress_notifications(&process_name) {
        eprintln!("[velos] notifications suppressed for '{process_name}' (post-fix restart)");
        return Ok(());
    }

    let config = load_global_config()?;

    let language = config
        .notifications
        .as_ref()
        .and_then(|n| n.language.as_deref())
        .unwrap_or("en");
    let i18n = I18n::new(language);

    let log_lines = fetch_recent_logs(&process_name).await;
    if log_lines.is_empty() {
        return Ok(());
    }

    let hostname = crate::commands::notify_crash::hostname();
    let timestamp = crate::commands::notify_crash::chrono_now();
    let cwd = resolve_process_cwd(&process_name).await;

    // Extract source references from stack traces
    let source_refs = analyzer::extract_source_refs(&log_lines);
    let cwd_path = std::path::Path::new(&cwd);
    let source_snippets: Vec<SourceSnippet> = source_refs
        .iter()
        .filter_map(|(file, line)| analyzer::read_source_context(file, *line, cwd_path, 5))
        .collect();

    let ctx = CrashContext {
        process_name: process_name.clone(),
        exit_code: 0, // process is still running
        hostname: hostname.clone(),
        timestamp: timestamp.clone(),
        cwd: cwd.clone(),
        logs: log_lines.clone(),
        source_snippets,
    };

    // Run AI analysis if configured
    let ai_analysis = run_ai_analysis(&config.ai, &ctx);

    // Create and save crash record (reuse CrashRecord for error too)
    let crash_id = uuid::Uuid::new_v4().to_string();
    let record = CrashRecord {
        id: crash_id.clone(),
        process_name: process_name.clone(),
        exit_code: 0,
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
        eprintln!("[velos] failed to save error record: {e}");
    }

    // Send Telegram notification
    let telegram = config.notifications.and_then(|n| n.telegram);
    if let Some(t) = telegram {
        if !t.bot_token.is_empty() && !t.chat_id.is_empty() {
            let text = build_telegram_message(
                &i18n,
                &process_name,
                &hostname,
                &timestamp,
                &log_lines,
                &ai_analysis,
            );
            if let Err(e) = send_telegram_with_buttons(
                &t.bot_token,
                &t.chat_id,
                &text,
                &crash_id,
                &i18n,
            ) {
                eprintln!("[velos] Telegram send error: {e}");
            }
        }
    }

    Ok(())
}

fn run_ai_analysis(
    ai_config: &Option<super::config::AiConfigToml>,
    ctx: &CrashContext,
) -> Option<String> {
    let ai = ai_config.as_ref()?;
    if !ai.auto_analyze || ai.provider.is_empty() || ai.api_key.is_empty() {
        return None;
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
            return None;
        }
    };

    match analyzer::analyze(provider.as_ref(), ctx) {
        Ok(analysis) => Some(analysis),
        Err(e) => {
            eprintln!("[velos] AI analysis error: {e}");
            None
        }
    }
}

fn build_telegram_message(
    i18n: &I18n,
    process_name: &str,
    hostname: &str,
    timestamp: &str,
    log_lines: &[String],
    ai_analysis: &Option<String>,
) -> String {
    let h = html_escape;
    let mut text = format!(
        "\u{26A0}\u{FE0F} <b>{}</b>\n\n\
         <b>{}:</b> <code>{}</code>\n\
         <b>{}:</b> <code>{}</code>\n\
         <b>{}:</b> {}",
        h(i18n.get("error.title")),
        h(i18n.get("crash.name")),
        h(process_name),
        h(i18n.get("crash.host")),
        h(hostname),
        h(i18n.get("crash.time")),
        h(timestamp),
    );

    if !log_lines.is_empty() {
        let log_tail: String = log_lines
            .iter()
            .rev()
            .take(15)
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

    if let Some(analysis) = ai_analysis {
        text.push_str(&format!(
            "\n\n<b>{}:</b>\n{}",
            h(i18n.get("crash.analysis_header")),
            h(analysis),
        ));
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
        let logs = client.logs(id, 30).await?;
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
