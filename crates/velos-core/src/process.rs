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
}

fn default_kill_timeout() -> u32 {
    5000
}

fn default_autorestart() -> bool {
    true
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
