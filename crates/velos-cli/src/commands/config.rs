use std::path::PathBuf;

use velos_core::VelosError;

/// Global daemon config at ~/.velos/config.toml
/// Separate from per-app velos.toml (VelosConfig).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub ai: Option<AiConfigToml>,
    #[serde(default)]
    pub notifications: Option<NotificationsConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct AiConfigToml {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_true")]
    pub auto_analyze: bool,
    #[serde(default)]
    pub auto_fix: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub language: Option<String>,
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

fn default_max_iterations() -> u32 { 30 }
fn default_true() -> bool { true }

pub fn config_path() -> PathBuf {
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

const AVAILABLE_KEYS: &str = "\
  ai.provider\n  ai.model\n  ai.api_key\n  ai.base_url\n  \
  ai.max_iterations\n  ai.auto_analyze\n  ai.auto_fix\n  \
  notifications.language\n  telegram.bot_token\n  telegram.chat_id";

pub async fn run_set(key: String, value: String) -> Result<(), VelosError> {
    let mut config = load_global_config()?;

    match key.as_str() {
        // AI settings
        "ai.provider" => {
            if value != "anthropic" && value != "openai" {
                return Err(VelosError::ProtocolError(
                    "provider must be 'anthropic' or 'openai'".into(),
                ));
            }
            config.ai.get_or_insert_with(Default::default).provider = value.clone();
        }
        "ai.model" => {
            config.ai.get_or_insert_with(Default::default).model = value.clone();
        }
        "ai.api_key" => {
            config.ai.get_or_insert_with(Default::default).api_key = value.clone();
        }
        "ai.base_url" => {
            config.ai.get_or_insert_with(Default::default).base_url = value.clone();
        }
        "ai.max_iterations" => {
            let n: u32 = value.parse().map_err(|_| {
                VelosError::ProtocolError("max_iterations must be a positive integer".into())
            })?;
            if n == 0 {
                return Err(VelosError::ProtocolError("max_iterations must be > 0".into()));
            }
            config.ai.get_or_insert_with(Default::default).max_iterations = n;
        }
        "ai.auto_analyze" => {
            let b = parse_bool(&value)?;
            config.ai.get_or_insert_with(Default::default).auto_analyze = b;
        }
        "ai.auto_fix" => {
            let b = parse_bool(&value)?;
            config.ai.get_or_insert_with(Default::default).auto_fix = b;
        }
        // Notification settings
        "notifications.language" => {
            let notif = config.notifications.get_or_insert_with(Default::default);
            notif.language = Some(value.clone());
        }
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
                "Unknown config key: {key}\n\nAvailable keys:\n{AVAILABLE_KEYS}"
            )));
        }
    }

    save_global_config(&config)?;
    println!("Set {key} = {}", mask_secret(&key, &value));
    Ok(())
}

pub async fn run_get(key: Option<String>) -> Result<(), VelosError> {
    let config = load_global_config()?;

    match key.as_deref() {
        // AI keys
        Some("ai.provider") => println!("{}", ai_field(&config, |a| &a.provider)),
        Some("ai.model") => println!("{}", ai_field(&config, |a| &a.model)),
        Some("ai.api_key") => println!("{}", mask_secret("ai.api_key", &ai_field(&config, |a| &a.api_key))),
        Some("ai.base_url") => println!("{}", ai_field(&config, |a| &a.base_url)),
        Some("ai.max_iterations") => {
            println!("{}", config.ai.as_ref().map(|a| a.max_iterations).unwrap_or(30));
        }
        Some("ai.auto_analyze") => {
            println!("{}", config.ai.as_ref().map(|a| a.auto_analyze).unwrap_or(true));
        }
        Some("ai.auto_fix") => {
            println!("{}", config.ai.as_ref().map(|a| a.auto_fix).unwrap_or(false));
        }
        // Notification keys
        Some("notifications.language") => {
            let lang = config.notifications.as_ref()
                .and_then(|n| n.language.as_deref())
                .unwrap_or("en");
            println!("{lang}");
        }
        Some("telegram.bot_token") => {
            let val = config.notifications.as_ref()
                .and_then(|n| n.telegram.as_ref())
                .map(|t| t.bot_token.as_str())
                .unwrap_or("");
            println!("{}", mask_secret("telegram.bot_token", val));
        }
        Some("telegram.chat_id") => {
            let val = config.notifications.as_ref()
                .and_then(|n| n.telegram.as_ref())
                .map(|t| t.chat_id.as_str())
                .unwrap_or("");
            println!("{val}");
        }
        Some(k) => {
            return Err(VelosError::ProtocolError(format!(
                "Unknown config key: {k}\n\nAvailable keys:\n{AVAILABLE_KEYS}"
            )));
        }
        None => {
            let path = config_path();
            println!("Config: {}\n", path.display());
            // AI section
            if let Some(ai) = &config.ai {
                println!("[ai]");
                println!("  provider       = {}", ai.provider);
                println!("  model          = {}", ai.model);
                println!("  api_key        = {}", mask_secret("api_key", &ai.api_key));
                if !ai.base_url.is_empty() {
                    println!("  base_url       = {}", ai.base_url);
                }
                println!("  max_iterations = {}", ai.max_iterations);
                println!("  auto_analyze   = {}", ai.auto_analyze);
                println!("  auto_fix       = {}", ai.auto_fix);
                println!();
            }
            // Notifications section
            if let Some(notif) = &config.notifications {
                if let Some(lang) = &notif.language {
                    println!("[notifications]");
                    println!("  language = {lang}");
                    println!();
                }
                if let Some(tg) = &notif.telegram {
                    println!("[notifications.telegram]");
                    println!("  bot_token = {}", mask_secret("bot_token", &tg.bot_token));
                    println!("  chat_id   = {}", tg.chat_id);
                    println!();
                }
            }
            if config.ai.is_none() && config.notifications.is_none() {
                println!("(empty config)");
            }
        }
    }
    Ok(())
}

fn ai_field(config: &GlobalConfig, f: impl Fn(&AiConfigToml) -> &str) -> String {
    config.ai.as_ref().map(|a| f(a).to_string()).unwrap_or_default()
}

/// Mask sensitive values (api_key, bot_token) for display.
fn mask_secret(key: &str, value: &str) -> String {
    if (!key.contains("key") && !key.contains("token")) || value.len() < 16 {
        return value.to_string();
    }
    let start = &value[..8];
    let end = &value[value.len() - 4..];
    format!("{start}...{end}")
}

fn parse_bool(s: &str) -> Result<bool, VelosError> {
    match s {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(VelosError::ProtocolError(format!(
            "expected true/false, got '{s}'"
        ))),
    }
}
