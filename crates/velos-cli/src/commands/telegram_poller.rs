use std::path::PathBuf;

use velos_ai::analyzer::{CrashRecord, CrashStatus};
use velos_ai::i18n::I18n;
use velos_core::VelosError;

use super::config::{load_global_config, AiConfigToml, TelegramConfig};

/// Run the Telegram callback poller (blocking).
/// Intended to run in a background thread from the daemon.
pub fn run_poller() -> Result<(), VelosError> {
    let config = load_global_config()?;
    let telegram = config
        .notifications
        .as_ref()
        .and_then(|n| n.telegram.as_ref())
        .filter(|t| !t.bot_token.is_empty() && !t.chat_id.is_empty())
        .cloned()
        .ok_or_else(|| VelosError::ProtocolError("Telegram not configured".into()))?;

    let language = config
        .notifications
        .as_ref()
        .and_then(|n| n.language.as_deref())
        .unwrap_or("en")
        .to_string();

    eprintln!("[velos] Telegram callback poller started");

    let mut offset: i64 = 0;

    loop {
        match poll_updates(&telegram.bot_token, offset) {
            Ok(updates) => {
                for update in updates {
                    if let Some(new_offset) = update.update_id {
                        offset = new_offset + 1;
                    }
                    if let Some(cb) = &update.callback_query {
                        // Run in catch_unwind so panics in AI agent don't kill the poller
                        let tg = telegram.clone();
                        let ai = config.ai.clone();
                        let lang = language.clone();
                        let cb_owned = serde_json::to_string(cb).unwrap_or_default();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if let Ok(cb_ref) = serde_json::from_str::<CallbackQuery>(&cb_owned) {
                                handle_callback(&tg, &ai, &lang, &cb_ref);
                            }
                        }));
                        if let Err(e) = result {
                            eprintln!("[velos] callback handler panicked: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[velos] Telegram poll error: {e}");
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct TelegramResponse {
    ok: bool,
    result: Option<Vec<Update>>,
}

#[derive(serde::Deserialize)]
struct Update {
    update_id: Option<i64>,
    callback_query: Option<CallbackQuery>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CallbackQuery {
    id: String,
    data: Option<String>,
    message: Option<CallbackMessage>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CallbackMessage {
    chat: Chat,
    message_id: Option<i64>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Chat {
    id: i64,
}

fn poll_updates(bot_token: &str, offset: i64) -> Result<Vec<Update>, VelosError> {
    let url = format!("https://api.telegram.org/bot{bot_token}/getUpdates");

    let resp = ureq::post(&url)
        .send_json(&serde_json::json!({
            "offset": offset,
            "timeout": 30,
            "allowed_updates": ["callback_query"]
        }))
        .map_err(|e| VelosError::ProtocolError(format!("Telegram poll: {e}")))?;

    let body: TelegramResponse = resp
        .into_body()
        .read_json()
        .map_err(|e| VelosError::ProtocolError(format!("Telegram parse: {e}")))?;

    if !body.ok {
        return Err(VelosError::ProtocolError("Telegram API returned ok=false".into()));
    }

    Ok(body.result.unwrap_or_default())
}

fn handle_callback(
    telegram: &TelegramConfig,
    ai_config: &Option<AiConfigToml>,
    language: &str,
    cb: &CallbackQuery,
) {
    let data = match &cb.data {
        Some(d) => d.as_str(),
        None => return,
    };

    let i18n = I18n::new(language);

    // Answer callback to remove "loading" spinner in Telegram
    let _ = answer_callback(&telegram.bot_token, &cb.id, "");

    let chat_id = cb
        .message
        .as_ref()
        .map(|m| m.chat.id)
        .unwrap_or(0);
    let message_id = cb.message.as_ref().and_then(|m| m.message_id);

    if let Some(crash_id) = data.strip_prefix("fix:") {
        handle_fix(telegram, ai_config, &i18n, crash_id, chat_id, message_id);
    } else if let Some(crash_id) = data.strip_prefix("ignore:") {
        handle_ignore(telegram, &i18n, crash_id, chat_id, message_id);
    }
}

fn handle_fix(
    telegram: &TelegramConfig,
    _ai_config: &Option<AiConfigToml>,
    i18n: &I18n,
    crash_id: &str,
    chat_id: i64,
    message_id: Option<i64>,
) {
    // Remove inline keyboard from original message
    if let Some(mid) = message_id {
        let _ = remove_inline_keyboard(&telegram.bot_token, chat_id, mid);
    }

    if CrashRecord::load(crash_id).is_err() {
        let _ = send_message(
            &telegram.bot_token,
            chat_id,
            &format!("<i>{}</i>", i18n.get("fix.no_crash_record")),
        );
        return;
    }

    let _ = send_message(
        &telegram.bot_token,
        chat_id,
        &format!("\u{1F527} {}", i18n.get("fix.started")),
    );

    // Spawn `velos ai fix <crash-id>` as subprocess with per-crash log file
    let crashes_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("crashes");
    let log_path = crashes_dir.join(format!("{crash_id}.log"));

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{274C} {}: {e}", i18n.get("fix.failed")),
            );
            return;
        }
    };

    let log_file = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[velos] failed to create log file: {e}");
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{274C} {}: {e}", i18n.get("fix.failed")),
            );
            return;
        }
    };

    eprintln!("[velos] spawning fix for {crash_id}, logs: {}", log_path.display());

    let result = std::process::Command::new(&exe)
        .args(["ai", "fix", crash_id])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file.try_clone().unwrap_or_else(|_| log_file)))
        .stderr(std::process::Stdio::from(
            std::fs::File::create(&log_path).unwrap_or_else(|_| {
                std::fs::OpenOptions::new().append(true).open(&log_path).unwrap()
            }),
        ))
        .status();

    // Reload record to check final status
    let record = CrashRecord::load(crash_id);

    match (result, record) {
        (Ok(status), Ok(record)) if status.success() && record.status == CrashStatus::Fixed => {
            let summary = truncate(record.fix_result.as_deref().unwrap_or(""), 3000);
            let msg = format!(
                "\u{2705} <b>{}</b>\n\n<b>{}:</b>\n{}",
                i18n.get("fix.completed"),
                i18n.get("fix.changes_summary"),
                html_escape(&summary),
            );
            let _ = send_message(&telegram.bot_token, chat_id, &msg);
        }
        (Ok(_), Ok(record)) => {
            let err = record.fix_result.as_deref().unwrap_or("unknown error");
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{274C} {}: {}", i18n.get("fix.failed"), html_escape(err)),
            );
        }
        (Err(e), _) => {
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{274C} {}: {e}", i18n.get("fix.failed")),
            );
        }
        _ => {
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{274C} {}", i18n.get("fix.failed")),
            );
        }
    }
}

fn handle_ignore(
    telegram: &TelegramConfig,
    i18n: &I18n,
    crash_id: &str,
    chat_id: i64,
    message_id: Option<i64>,
) {
    if let Some(mid) = message_id {
        let _ = remove_inline_keyboard(&telegram.bot_token, chat_id, mid);
    }

    match CrashRecord::load(crash_id) {
        Ok(mut record) => {
            record.status = CrashStatus::Ignored;
            let _ = record.save();
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("\u{1F6AB} Crash <code>{}</code> — {}", crash_id, i18n.get("crash.btn_ignore").to_lowercase()),
            );
        }
        Err(_) => {
            let _ = send_message(
                &telegram.bot_token,
                chat_id,
                &format!("<i>{}</i>", i18n.get("fix.no_crash_record")),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Telegram helpers
// ---------------------------------------------------------------------------

fn answer_callback(bot_token: &str, callback_id: &str, text: &str) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{bot_token}/answerCallbackQuery");
    ureq::post(&url)
        .send_json(&serde_json::json!({
            "callback_query_id": callback_id,
            "text": text,
        }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn send_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    ureq::post(&url)
        .send_json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
        }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn remove_inline_keyboard(bot_token: &str, chat_id: i64, message_id: i64) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{bot_token}/editMessageReplyMarkup");
    ureq::post(&url)
        .send_json(&serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
