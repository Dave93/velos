use serde::{Deserialize, Serialize};

/// Extended process configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub name: String,
    pub script: String,
    pub cwd: Option<String>,
    pub interpreter: Option<String>,
    /// Kill timeout in ms (default 5000)
    #[serde(default = "default_kill_timeout")]
    pub kill_timeout_ms: u32,
    /// Auto-restart on crash (default true)
    #[serde(default = "default_autorestart")]
    pub autorestart: bool,
    /// Max restart attempts (-1 = unlimited, default 15)
    #[serde(default = "default_max_restarts")]
    pub max_restarts: i32,
    /// Minimum uptime in ms before considering process "stable" (default 1000)
    #[serde(default = "default_min_uptime_ms")]
    pub min_uptime_ms: u64,
    /// Delay between restarts in ms (default 0)
    #[serde(default)]
    pub restart_delay_ms: u32,
    /// Use exponential backoff for restart delays (default false)
    #[serde(default)]
    pub exp_backoff_restart_delay: bool,
    /// Restart process when memory exceeds this limit (bytes, None = unlimited)
    #[serde(default)]
    pub max_memory_restart: Option<u64>,
}

fn default_kill_timeout() -> u32 {
    5000
}

fn default_autorestart() -> bool {
    true
}

fn default_max_restarts() -> i32 {
    15
}

fn default_min_uptime_ms() -> u64 {
    1000
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            script: String::new(),
            cwd: None,
            interpreter: None,
            kill_timeout_ms: default_kill_timeout(),
            autorestart: default_autorestart(),
            max_restarts: default_max_restarts(),
            min_uptime_ms: default_min_uptime_ms(),
            restart_delay_ms: 0,
            exp_backoff_restart_delay: false,
            max_memory_restart: None,
        }
    }
}

/// Process lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    Starting,
    Online,
    Stopping,
    Stopped,
    Errored,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Online => write!(f, "online"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Errored => write!(f, "errored"),
        }
    }
}
