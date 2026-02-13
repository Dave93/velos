pub mod anomaly;
pub mod classifier;
pub mod dedup;
pub mod format;
pub mod pattern;
pub mod summary;

use velos_core::LogEntry;

/// Log level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
    Fatal = 4,
}

impl LogLevel {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Debug,
            2 => Self::Warn,
            3 => Self::Error,
            4 => Self::Fatal,
            _ => Self::Info,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }
}

/// A log entry enriched by the pipeline (with classified level).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessedEntry {
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub stream: u8,
    pub message: String,
}

impl ProcessedEntry {
    pub fn from_raw(entry: &LogEntry, level: LogLevel) -> Self {
        Self {
            timestamp_ms: entry.timestamp_ms,
            level,
            stream: entry.stream,
            message: entry.message.clone(),
        }
    }
}

/// Pipeline stage trait.
pub trait LogProcessor {
    fn process(&mut self, entries: &[ProcessedEntry]) -> Vec<ProcessedEntry>;
}

/// Configurable processing pipeline: chains multiple LogProcessors.
pub struct Pipeline {
    stages: Vec<Box<dyn LogProcessor>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn add_stage(&mut self, stage: Box<dyn LogProcessor>) {
        self.stages.push(stage);
    }

    pub fn run(&mut self, entries: &[ProcessedEntry]) -> Vec<ProcessedEntry> {
        let mut current = entries.to_vec();
        for stage in &mut self.stages {
            current = stage.process(&current);
        }
        current
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
