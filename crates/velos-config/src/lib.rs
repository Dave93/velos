use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;
use velos_core::ProcessConfig;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("app not found in config: {0}")]
    AppNotFound(String),

    #[error("invalid memory format: {0}")]
    InvalidMemory(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

// ---------------------------------------------------------------------------
// TOML data model
// ---------------------------------------------------------------------------

/// Configuration for the smart log engine pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct LogEngineConfig {
    /// Enable auto-classifier (default: true).
    #[serde(default = "default_true")]
    pub classifier: bool,
    /// Dedup sliding window in seconds (default: 60).
    #[serde(default = "default_dedup_window")]
    pub dedup_window: u64,
    /// Pattern detection time window in seconds (default: 300).
    #[serde(default = "default_pattern_window")]
    pub pattern_window: u64,
    /// Anomaly detection window size in minutes (default: 60).
    #[serde(default = "default_anomaly_window")]
    pub anomaly_window: u64,
    /// Sigma threshold for anomaly warning (default: 2.0).
    #[serde(default = "default_sigma_warn")]
    pub anomaly_sigma_warn: f64,
    /// Sigma threshold for anomaly critical (default: 3.0).
    #[serde(default = "default_sigma_crit")]
    pub anomaly_sigma_crit: f64,
}

impl Default for LogEngineConfig {
    fn default() -> Self {
        Self {
            classifier: default_true(),
            dedup_window: default_dedup_window(),
            pattern_window: default_pattern_window(),
            anomaly_window: default_anomaly_window(),
            anomaly_sigma_warn: default_sigma_warn(),
            anomaly_sigma_crit: default_sigma_crit(),
        }
    }
}

/// Top-level TOML config file (`velos.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct VelosConfig {
    /// Per-app configs, keyed by app name.
    #[serde(default)]
    pub apps: HashMap<String, AppConfig>,
    /// Log engine pipeline configuration.
    #[serde(default)]
    pub logs: Option<LogEngineConfig>,
}

/// Configuration for a single application.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Process name (defaults to the TOML key if omitted).
    pub name: Option<String>,
    /// Path to the script or binary to execute.
    pub script: String,
    /// Working directory.
    pub cwd: Option<String>,
    /// Interpreter (e.g. "node", "python3"). None = auto-detect or run directly.
    pub interpreter: Option<String>,
    /// Script arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Number of instances (1 = fork mode).
    #[serde(default = "default_instances")]
    pub instances: u32,
    /// Auto-restart on crash.
    #[serde(default = "default_true")]
    pub autorestart: bool,
    /// Max restart attempts (-1 = unlimited).
    #[serde(default = "default_max_restarts")]
    pub max_restarts: i32,
    /// Minimum uptime in ms to be considered stable.
    #[serde(default = "default_min_uptime")]
    pub min_uptime: u64,
    /// Delay between restarts in ms.
    #[serde(default)]
    pub restart_delay: u64,
    /// Enable exponential backoff for restart delay.
    #[serde(default)]
    pub exp_backoff_restart_delay: bool,
    /// Kill timeout in ms (SIGTERM → wait → SIGKILL).
    #[serde(default = "default_kill_timeout")]
    pub kill_timeout: u64,
    /// Max memory before forced restart (human-readable, e.g. "150M").
    pub max_memory_restart: Option<String>,
    /// Enable file watching.
    #[serde(default)]
    pub watch: bool,
    /// Paths to watch (relative to cwd).
    #[serde(default)]
    pub watch_paths: Vec<String>,
    /// Patterns to ignore when watching.
    #[serde(default)]
    pub watch_ignore: Vec<String>,
    /// Watch debounce delay in ms.
    #[serde(default = "default_watch_delay")]
    pub watch_delay: u64,
    /// Max log file size (human-readable, e.g. "10M").
    pub log_max_size: Option<String>,
    /// Number of rotated log files to keep.
    #[serde(default = "default_log_retain")]
    pub log_retain_count: u32,
    /// Cron expression for periodic restarts.
    #[serde(default)]
    pub cron_restart: Option<String>,
    /// Custom log file path.
    pub log_file: Option<String>,
    /// Merge stdout and stderr into a single log.
    #[serde(default)]
    pub merge_logs: bool,

    /// Base environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Environment profile overrides (e.g. env_production, env_staging).
    #[serde(flatten, deserialize_with = "deserialize_env_profiles")]
    pub env_profiles: HashMap<String, HashMap<String, String>>,
}

// ---------------------------------------------------------------------------
// Serde defaults
// ---------------------------------------------------------------------------

fn default_instances() -> u32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_max_restarts() -> i32 {
    15
}
fn default_min_uptime() -> u64 {
    1000
}
fn default_kill_timeout() -> u64 {
    5000
}
fn default_watch_delay() -> u64 {
    1000
}
fn default_log_retain() -> u32 {
    30
}
fn default_dedup_window() -> u64 {
    60
}
fn default_pattern_window() -> u64 {
    300
}
fn default_anomaly_window() -> u64 {
    60
}
fn default_sigma_warn() -> f64 {
    2.0
}
fn default_sigma_crit() -> f64 {
    3.0
}

// ---------------------------------------------------------------------------
// Custom deserializer for env_* profile fields
// ---------------------------------------------------------------------------

fn deserialize_env_profiles<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, HashMap<String, String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let map: HashMap<String, toml::Value> = HashMap::deserialize(deserializer)?;
    let mut profiles = HashMap::new();
    for (key, value) in map {
        if let Some(profile_name) = key.strip_prefix("env_") {
            if let toml::Value::Table(table) = value {
                let env_map: HashMap<String, String> = table
                    .into_iter()
                    .filter_map(|(k, v)| {
                        let s = match v {
                            toml::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        Some((k, s))
                    })
                    .collect();
                profiles.insert(profile_name.to_string(), env_map);
            }
        }
    }
    Ok(profiles)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load and parse a TOML config file.
pub fn load(path: &Path) -> Result<VelosConfig> {
    let content = std::fs::read_to_string(path)?;
    parse(&content)
}

/// Load a TOML config file and apply an environment profile.
pub fn load_with_env(path: &Path, env_profile: &str) -> Result<VelosConfig> {
    let mut config = load(path)?;
    for app in config.apps.values_mut() {
        apply_env_profile(app, env_profile);
    }
    Ok(config)
}

/// Parse TOML string into a VelosConfig.
pub fn parse(toml_str: &str) -> Result<VelosConfig> {
    let mut config: VelosConfig = toml::from_str(toml_str)?;

    // Back-fill name from TOML key if not explicitly set.
    for (key, app) in config.apps.iter_mut() {
        if app.name.is_none() {
            app.name = Some(key.clone());
        }
    }

    // Validate all apps.
    for (key, app) in &config.apps {
        validate_app(key, app)?;
    }

    Ok(config)
}

impl VelosConfig {
    /// Get config for a specific app by name.
    pub fn get_app(&self, name: &str) -> Option<&AppConfig> {
        self.apps.get(name)
    }

    /// Get all app configs as a vec of references.
    pub fn all_apps(&self) -> Vec<&AppConfig> {
        self.apps.values().collect()
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_app(key: &str, app: &AppConfig) -> Result<()> {
    let name = app.name.as_deref().unwrap_or(key);

    // Name must be non-empty, alphanumeric + dash + underscore.
    if name.is_empty() {
        return Err(ConfigError::Validation(
            "app name must not be empty".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ConfigError::Validation(format!(
            "app name '{}' contains invalid characters (only alphanumeric, dash, underscore allowed)",
            name
        )));
    }

    // Script must be non-empty.
    if app.script.is_empty() {
        return Err(ConfigError::Validation(format!(
            "app '{}': script must not be empty",
            name
        )));
    }

    // max_restarts >= -1.
    if app.max_restarts < -1 {
        return Err(ConfigError::Validation(format!(
            "app '{}': max_restarts must be >= -1, got {}",
            name, app.max_restarts
        )));
    }

    // kill_timeout >= 100.
    if app.kill_timeout < 100 {
        return Err(ConfigError::Validation(format!(
            "app '{}': kill_timeout must be >= 100ms, got {}",
            name, app.kill_timeout
        )));
    }

    // Validate max_memory_restart if present.
    if let Some(ref mem) = app.max_memory_restart {
        parse_memory_string(mem).map_err(|_| {
            ConfigError::Validation(format!(
                "app '{}': invalid max_memory_restart format: '{}'",
                name, mem
            ))
        })?;
    }

    // Validate log_max_size if present.
    if let Some(ref size) = app.log_max_size {
        parse_memory_string(size).map_err(|_| {
            ConfigError::Validation(format!(
                "app '{}': invalid log_max_size format: '{}'",
                name, size
            ))
        })?;
    }

    // Basic cron validation (5 space-separated fields).
    if let Some(ref cron) = app.cron_restart {
        if !cron.is_empty() {
            let fields: Vec<&str> = cron.split_whitespace().collect();
            if fields.len() != 5 {
                return Err(ConfigError::Validation(format!(
                    "app '{}': cron_restart must have 5 fields, got {}",
                    name,
                    fields.len()
                )));
            }
        }
    }

    // instances must be >= 1.
    if app.instances < 1 {
        return Err(ConfigError::Validation(format!(
            "app '{}': instances must be >= 1, got {}",
            name, app.instances
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Memory string parsing
// ---------------------------------------------------------------------------

/// Parse a human-readable memory string (e.g. "150M", "1G", "512K") into bytes.
pub fn parse_memory_string(s: &str) -> std::result::Result<u64, ConfigError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ConfigError::InvalidMemory("empty string".into()));
    }

    // Try to find suffix.
    let (num_part, multiplier) = if let Some(num) = s.strip_suffix('G') {
        (num, 1024u64 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('g') {
        (num, 1024u64 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        (num, 1024u64 * 1024)
    } else if let Some(num) = s.strip_suffix('m') {
        (num, 1024u64 * 1024)
    } else if let Some(num) = s.strip_suffix('K') {
        (num, 1024u64)
    } else if let Some(num) = s.strip_suffix('k') {
        (num, 1024u64)
    } else if let Some(num) = s.strip_suffix('B') {
        (num, 1u64)
    } else if let Some(num) = s.strip_suffix('b') {
        (num, 1u64)
    } else {
        // No suffix — assume bytes.
        (s, 1u64)
    };

    let num: u64 = num_part
        .trim()
        .parse()
        .map_err(|_| ConfigError::InvalidMemory(format!("cannot parse number: '{}'", num_part)))?;

    Ok(num * multiplier)
}

// ---------------------------------------------------------------------------
// Env profile merging
// ---------------------------------------------------------------------------

/// Apply an environment profile to an app config.
/// The profile vars override the base `env` section.
fn apply_env_profile(app: &mut AppConfig, profile: &str) {
    if let Some(profile_env) = app.env_profiles.get(profile) {
        for (k, v) in profile_env {
            app.env.insert(k.clone(), v.clone());
        }
    }
}

/// Merge an app's env with a specific profile, returning the resulting HashMap.
pub fn merged_env(app: &AppConfig, profile: Option<&str>) -> HashMap<String, String> {
    let mut env = app.env.clone();
    if let Some(profile) = profile {
        if let Some(profile_env) = app.env_profiles.get(profile) {
            for (k, v) in profile_env {
                env.insert(k.clone(), v.clone());
            }
        }
    }
    env
}

// ---------------------------------------------------------------------------
// CLI merge: AppConfig → ProcessConfig
// ---------------------------------------------------------------------------

/// Options from CLI that can override config values.
#[derive(Debug, Default)]
pub struct CliOverrides {
    pub name: Option<String>,
    pub script: Option<String>,
    pub cwd: Option<String>,
    pub interpreter: Option<String>,
    pub kill_timeout_ms: Option<u32>,
    pub autorestart: Option<bool>,
    pub max_restarts: Option<i32>,
    pub max_memory_restart: Option<u64>,
}

/// Convert an AppConfig into a ProcessConfig, applying CLI overrides.
pub fn merge_with_cli(app: &AppConfig, overrides: &CliOverrides) -> ProcessConfig {
    let max_memory = overrides.max_memory_restart.or_else(|| {
        app.max_memory_restart
            .as_deref()
            .and_then(|s| parse_memory_string(s).ok())
    });

    ProcessConfig {
        name: overrides
            .name
            .clone()
            .or_else(|| app.name.clone())
            .unwrap_or_default(),
        script: overrides
            .script
            .clone()
            .unwrap_or_else(|| app.script.clone()),
        cwd: overrides.cwd.clone().or_else(|| app.cwd.clone()),
        interpreter: overrides
            .interpreter
            .clone()
            .or_else(|| app.interpreter.clone()),
        kill_timeout_ms: overrides.kill_timeout_ms.unwrap_or(app.kill_timeout as u32),
        autorestart: overrides.autorestart.unwrap_or(app.autorestart),
        max_restarts: overrides.max_restarts.unwrap_or(app.max_restarts),
        min_uptime_ms: app.min_uptime,
        restart_delay_ms: app.restart_delay as u32,
        exp_backoff_restart_delay: app.exp_backoff_restart_delay,
        max_memory_restart: max_memory,
    }
}

/// Find an app in the config by name and convert it to ProcessConfig.
pub fn resolve_app(
    config: &VelosConfig,
    app_name: &str,
    overrides: &CliOverrides,
    env_profile: Option<&str>,
) -> Result<ProcessConfig> {
    let app = config
        .apps
        .get(app_name)
        .ok_or_else(|| ConfigError::AppNotFound(app_name.to_string()))?;

    // If an env profile is requested, create a modified copy with merged env.
    let _ = merged_env(app, env_profile);

    Ok(merge_with_cli(app, overrides))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_TOML: &str = r#"
[apps.api]
script = "server.js"
cwd = "/app"
interpreter = "node"

[apps.api.env]
NODE_ENV = "production"
PORT = "3000"

[apps.api.env_production]
NODE_ENV = "production"
DATABASE_URL = "postgres://prod:5432/db"

[apps.api.env_development]
NODE_ENV = "development"
DATABASE_URL = "postgres://localhost:5432/db"
"#;

    #[test]
    fn parse_valid_toml() {
        let config = parse(BASIC_TOML).unwrap();
        assert_eq!(config.apps.len(), 1);

        let api = config.get_app("api").unwrap();
        assert_eq!(api.name.as_deref(), Some("api"));
        assert_eq!(api.script, "server.js");
        assert_eq!(api.cwd.as_deref(), Some("/app"));
        assert_eq!(api.interpreter.as_deref(), Some("node"));
        assert_eq!(api.autorestart, true);
        assert_eq!(api.max_restarts, 15);
        assert_eq!(api.kill_timeout, 5000);
        assert_eq!(api.instances, 1);
    }

    #[test]
    fn parse_env_vars() {
        let config = parse(BASIC_TOML).unwrap();
        let api = config.get_app("api").unwrap();

        assert_eq!(api.env.get("NODE_ENV").unwrap(), "production");
        assert_eq!(api.env.get("PORT").unwrap(), "3000");
    }

    #[test]
    fn env_profile_merge() {
        let config = parse(BASIC_TOML).unwrap();
        let api = config.get_app("api").unwrap();

        // Base env.
        let base = merged_env(api, None);
        assert_eq!(base.get("NODE_ENV").unwrap(), "production");
        assert_eq!(base.get("PORT").unwrap(), "3000");
        assert!(base.get("DATABASE_URL").is_none());

        // Production profile: adds DATABASE_URL, keeps PORT from base.
        let prod = merged_env(api, Some("production"));
        assert_eq!(prod.get("NODE_ENV").unwrap(), "production");
        assert_eq!(prod.get("PORT").unwrap(), "3000");
        assert_eq!(
            prod.get("DATABASE_URL").unwrap(),
            "postgres://prod:5432/db"
        );

        // Development profile: overrides NODE_ENV, adds DATABASE_URL.
        let dev = merged_env(api, Some("development"));
        assert_eq!(dev.get("NODE_ENV").unwrap(), "development");
        assert_eq!(
            dev.get("DATABASE_URL").unwrap(),
            "postgres://localhost:5432/db"
        );
    }

    #[test]
    fn parse_memory_strings() {
        assert_eq!(parse_memory_string("512K").unwrap(), 512 * 1024);
        assert_eq!(parse_memory_string("150M").unwrap(), 150 * 1024 * 1024);
        assert_eq!(
            parse_memory_string("1G").unwrap(),
            1024 * 1024 * 1024
        );
        assert_eq!(parse_memory_string("1024").unwrap(), 1024);
        assert_eq!(parse_memory_string("100B").unwrap(), 100);
        assert_eq!(parse_memory_string("2g").unwrap(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn parse_memory_invalid() {
        assert!(parse_memory_string("").is_err());
        assert!(parse_memory_string("abc").is_err());
        assert!(parse_memory_string("M").is_err());
    }

    #[test]
    fn validate_empty_name() {
        let toml_str = r#"
[apps.""]
script = "server.js"
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("name must not be empty"));
    }

    #[test]
    fn validate_invalid_name_chars() {
        let toml_str = r#"
[apps."my app"]
script = "server.js"
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn validate_empty_script() {
        let toml_str = r#"
[apps.api]
script = ""
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("script must not be empty"));
    }

    #[test]
    fn validate_kill_timeout_too_low() {
        let toml_str = r#"
[apps.api]
script = "server.js"
kill_timeout = 50
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("kill_timeout must be >= 100"));
    }

    #[test]
    fn validate_max_restarts_too_low() {
        let toml_str = r#"
[apps.api]
script = "server.js"
max_restarts = -2
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("max_restarts must be >= -1"));
    }

    #[test]
    fn validate_invalid_memory_format() {
        let toml_str = r#"
[apps.api]
script = "server.js"
max_memory_restart = "notanumber"
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("invalid max_memory_restart"));
    }

    #[test]
    fn validate_invalid_cron() {
        let toml_str = r#"
[apps.api]
script = "server.js"
cron_restart = "* *"
"#;
        let err = parse(toml_str).unwrap_err();
        assert!(err.to_string().contains("cron_restart must have 5 fields"));
    }

    #[test]
    fn merge_with_cli_overrides() {
        let config = parse(BASIC_TOML).unwrap();
        let api = config.get_app("api").unwrap();

        // No overrides: values come from config.
        let pc = merge_with_cli(api, &CliOverrides::default());
        assert_eq!(pc.name, "api");
        assert_eq!(pc.script, "server.js");
        assert_eq!(pc.cwd.as_deref(), Some("/app"));
        assert_eq!(pc.interpreter.as_deref(), Some("node"));
        assert_eq!(pc.kill_timeout_ms, 5000);
        assert!(pc.autorestart);

        // CLI overrides some values.
        let overrides = CliOverrides {
            name: Some("api-v2".into()),
            script: Some("server-v2.js".into()),
            cwd: Some("/app/v2".into()),
            kill_timeout_ms: Some(10000),
            ..Default::default()
        };
        let pc = merge_with_cli(api, &overrides);
        assert_eq!(pc.name, "api-v2");
        assert_eq!(pc.script, "server-v2.js");
        assert_eq!(pc.cwd.as_deref(), Some("/app/v2"));
        assert_eq!(pc.interpreter.as_deref(), Some("node")); // not overridden
        assert_eq!(pc.kill_timeout_ms, 10000);
    }

    #[test]
    fn resolve_app_not_found() {
        let config = parse(BASIC_TOML).unwrap();
        let err = resolve_app(&config, "nonexistent", &CliOverrides::default(), None).unwrap_err();
        assert!(matches!(err, ConfigError::AppNotFound(_)));
    }

    #[test]
    fn all_apps_returns_all() {
        let toml_str = r#"
[apps.web]
script = "web.js"

[apps.worker]
script = "worker.js"
"#;
        let config = parse(toml_str).unwrap();
        assert_eq!(config.all_apps().len(), 2);
    }

    #[test]
    fn multi_app_config() {
        let toml_str = r#"
[apps.web]
script = "web.js"
interpreter = "node"
instances = 4
max_memory_restart = "256M"

[apps.worker]
script = "worker.py"
interpreter = "python3"
instances = 2
autorestart = false
"#;
        let config = parse(toml_str).unwrap();

        let web = config.get_app("web").unwrap();
        assert_eq!(web.instances, 4);
        assert_eq!(
            parse_memory_string(web.max_memory_restart.as_deref().unwrap()).unwrap(),
            256 * 1024 * 1024
        );

        let worker = config.get_app("worker").unwrap();
        assert_eq!(worker.instances, 2);
        assert_eq!(worker.interpreter.as_deref(), Some("python3"));
        assert!(!worker.autorestart);
    }

    #[test]
    fn watch_config() {
        let toml_str = r#"
[apps.api]
script = "server.js"
watch = true
watch_paths = ["src/", "config/"]
watch_ignore = ["node_modules", ".git"]
watch_delay = 2000
"#;
        let config = parse(toml_str).unwrap();
        let api = config.get_app("api").unwrap();
        assert!(api.watch);
        assert_eq!(api.watch_paths, vec!["src/", "config/"]);
        assert_eq!(api.watch_ignore, vec!["node_modules", ".git"]);
        assert_eq!(api.watch_delay, 2000);
    }

    #[test]
    fn defaults_are_applied() {
        let toml_str = r#"
[apps.minimal]
script = "app.js"
"#;
        let config = parse(toml_str).unwrap();
        let app = config.get_app("minimal").unwrap();
        assert!(app.autorestart);
        assert_eq!(app.max_restarts, 15);
        assert_eq!(app.min_uptime, 1000);
        assert_eq!(app.restart_delay, 0);
        assert_eq!(app.kill_timeout, 5000);
        assert_eq!(app.instances, 1);
        assert!(!app.watch);
        assert_eq!(app.watch_delay, 1000);
        assert_eq!(app.log_retain_count, 30);
        assert!(!app.merge_logs);
        assert!(!app.exp_backoff_restart_delay);
    }

    #[test]
    fn load_with_env_applies_profile() {
        let toml_str = BASIC_TOML;
        // Write to a temp file.
        let dir = std::env::temp_dir().join("velos_config_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_env.toml");
        std::fs::write(&path, toml_str).unwrap();

        let config = load_with_env(&path, "development").unwrap();
        let api = config.get_app("api").unwrap();
        assert_eq!(api.env.get("NODE_ENV").unwrap(), "development");
        assert_eq!(
            api.env.get("DATABASE_URL").unwrap(),
            "postgres://localhost:5432/db"
        );
        // PORT still comes from base.
        assert_eq!(api.env.get("PORT").unwrap(), "3000");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_cron_passes() {
        let toml_str = r#"
[apps.api]
script = "server.js"
cron_restart = "0 0 * * *"
"#;
        let config = parse(toml_str).unwrap();
        let api = config.get_app("api").unwrap();
        assert_eq!(api.cron_restart.as_deref(), Some("0 0 * * *"));
    }

    #[test]
    fn valid_name_with_dashes_underscores() {
        let toml_str = r#"
[apps.my-app_v2]
script = "app.js"
"#;
        let config = parse(toml_str).unwrap();
        assert!(config.get_app("my-app_v2").is_some());
    }

    #[test]
    fn parse_log_engine_config() {
        let toml_str = r#"
[logs]
classifier = true
dedup_window = 120
pattern_window = 600
anomaly_window = 30
anomaly_sigma_warn = 1.5
anomaly_sigma_crit = 2.5

[apps.api]
script = "server.js"
"#;
        let config = parse(toml_str).unwrap();
        let logs = config.logs.unwrap();
        assert!(logs.classifier);
        assert_eq!(logs.dedup_window, 120);
        assert_eq!(logs.pattern_window, 600);
        assert_eq!(logs.anomaly_window, 30);
        assert!((logs.anomaly_sigma_warn - 1.5).abs() < f64::EPSILON);
        assert!((logs.anomaly_sigma_crit - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_log_engine_defaults() {
        let toml_str = r#"
[apps.api]
script = "server.js"
"#;
        let config = parse(toml_str).unwrap();
        assert!(config.logs.is_none());

        // Test default values
        let defaults = LogEngineConfig::default();
        assert!(defaults.classifier);
        assert_eq!(defaults.dedup_window, 60);
        assert_eq!(defaults.pattern_window, 300);
        assert_eq!(defaults.anomaly_window, 60);
        assert!((defaults.anomaly_sigma_warn - 2.0).abs() < f64::EPSILON);
        assert!((defaults.anomaly_sigma_crit - 3.0).abs() < f64::EPSILON);
    }
}
