use std::path::PathBuf;

use velos_core::VelosError;

/// Global daemon config at ~/.velos/config.toml
/// Separate from per-app velos.toml (VelosConfig).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub notifications: Option<NotificationsConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("config.toml")
}

pub fn load_global_config() -> Result<GlobalConfig, VelosError> {
    let path = config_path();
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| VelosError::Io(e))?;
    toml::from_str(&content)
        .map_err(|e| VelosError::ProtocolError(format!("config parse error: {e}")))
}

fn save_global_config(config: &GlobalConfig) -> Result<(), VelosError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(VelosError::Io)?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| VelosError::ProtocolError(format!("config serialize error: {e}")))?;
    std::fs::write(&path, content).map_err(VelosError::Io)?;
    Ok(())
}

pub async fn run_set(key: String, value: String) -> Result<(), VelosError> {
    let mut config = load_global_config()?;

    match key.as_str() {
        "telegram.bot_token" => {
            let notif = config.notifications.get_or_insert_with(Default::default);
            let tg = notif.telegram.get_or_insert_with(Default::default);
            tg.bot_token = value.clone();
        }
        "telegram.chat_id" => {
            let notif = config.notifications.get_or_insert_with(Default::default);
            let tg = notif.telegram.get_or_insert_with(Default::default);
            tg.chat_id = value.clone();
        }
        _ => {
            return Err(VelosError::ProtocolError(format!(
                "Unknown config key: {key}\n\nAvailable keys:\n  telegram.bot_token\n  telegram.chat_id"
            )));
        }
    }

    save_global_config(&config)?;
    println!("Set {key} = {}", mask_token(&key, &value));
    Ok(())
}

pub async fn run_get(key: Option<String>) -> Result<(), VelosError> {
    let config = load_global_config()?;

    match key.as_deref() {
        Some("telegram.bot_token") => {
            let val = config
                .notifications
                .as_ref()
                .and_then(|n| n.telegram.as_ref())
                .map(|t| t.bot_token.as_str())
                .unwrap_or("");
            println!("{}", mask_token("telegram.bot_token", val));
        }
        Some("telegram.chat_id") => {
            let val = config
                .notifications
                .as_ref()
                .and_then(|n| n.telegram.as_ref())
                .map(|t| t.chat_id.as_str())
                .unwrap_or("");
            println!("{val}");
        }
        Some(k) => {
            return Err(VelosError::ProtocolError(format!(
                "Unknown config key: {k}\n\nAvailable keys:\n  telegram.bot_token\n  telegram.chat_id"
            )));
        }
        None => {
            // Show all config
            let path = config_path();
            println!("Config: {}", path.display());
            println!();
            if let Some(notif) = &config.notifications {
                if let Some(tg) = &notif.telegram {
                    println!("[notifications.telegram]");
                    println!("  bot_token = {}", mask_token("telegram.bot_token", &tg.bot_token));
                    println!("  chat_id   = {}", tg.chat_id);
                } else {
                    println!("(no notifications configured)");
                }
            } else {
                println!("(no notifications configured)");
            }
        }
    }
    Ok(())
}

/// Mask sensitive tokens for display (show first 8 and last 4 chars)
fn mask_token(key: &str, value: &str) -> String {
    if !key.contains("token") || value.len() < 16 {
        return value.to_string();
    }
    let start = &value[..8];
    let end = &value[value.len() - 4..];
    format!("{start}...{end}")
}
