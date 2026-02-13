use regex::Regex;
use velos_core::LogEntry;

use crate::{LogLevel, ProcessedEntry};

/// A single classification rule: regex pattern → log level.
pub struct ClassificationRule {
    pub pattern: Regex,
    pub level: LogLevel,
    pub priority: u8,
}

/// Auto-classifies raw log entries by detecting log level from message content.
pub struct Classifier {
    rules: Vec<ClassificationRule>,
}

impl Classifier {
    /// Create a classifier with the default ruleset from ARCHITECTURE.md 5.2.
    pub fn with_defaults() -> Self {
        let rules = vec![
            ClassificationRule {
                pattern: Regex::new(r"(?i)\b(fatal|panic|critical)\b").unwrap(),
                level: LogLevel::Fatal,
                priority: 10,
            },
            ClassificationRule {
                pattern: Regex::new(r"(?i)\b(error|err|exception|fail(ed|ure)?)\b").unwrap(),
                level: LogLevel::Error,
                priority: 8,
            },
            ClassificationRule {
                pattern: Regex::new(r"(?i)\b(warn(ing)?|deprecated)\b").unwrap(),
                level: LogLevel::Warn,
                priority: 6,
            },
            ClassificationRule {
                pattern: Regex::new(r"(?i)\b(debug|trace|verbose)\b").unwrap(),
                level: LogLevel::Debug,
                priority: 4,
            },
        ];
        Self { rules }
    }

    /// Create an empty classifier (no rules, everything is Info).
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a custom rule.
    pub fn add_rule(&mut self, pattern: &str, level: LogLevel, priority: u8) {
        if let Ok(re) = Regex::new(pattern) {
            self.rules.push(ClassificationRule {
                pattern: re,
                level,
                priority,
            });
            self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
    }

    /// Classify a single log entry.
    pub fn classify(&self, entry: &LogEntry) -> LogLevel {
        // If the daemon already assigned a non-default level, trust it
        if entry.level != 1 {
            return LogLevel::from_u8(entry.level);
        }

        // JSON-aware: try to parse as JSON and extract "level" field
        if entry.message.starts_with('{') {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&entry.message) {
                if let Some(lvl) = val.get("level").and_then(|v| v.as_str()) {
                    return match lvl.to_lowercase().as_str() {
                        "fatal" | "panic" | "critical" => LogLevel::Fatal,
                        "error" | "err" => LogLevel::Error,
                        "warn" | "warning" => LogLevel::Warn,
                        "debug" | "trace" => LogLevel::Debug,
                        _ => LogLevel::Info,
                    };
                }
            }
        }

        // Apply rules (sorted by priority)
        for rule in &self.rules {
            if rule.pattern.is_match(&entry.message) {
                // stderr floor: if on stderr, level is at least Warn
                if entry.stream == 1 && (rule.level as u8) < (LogLevel::Warn as u8) {
                    return LogLevel::Warn;
                }
                return rule.level;
            }
        }

        // Default: stderr → Warn, stdout → Info
        if entry.stream == 1 {
            LogLevel::Warn
        } else {
            LogLevel::Info
        }
    }

    /// Classify a batch of raw LogEntry into ProcessedEntry.
    pub fn classify_batch(&self, entries: &[LogEntry]) -> Vec<ProcessedEntry> {
        entries
            .iter()
            .map(|e| ProcessedEntry::from_raw(e, self.classify(e)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(msg: &str, level: u8, stream: u8) -> LogEntry {
        LogEntry {
            timestamp_ms: 1000,
            level,
            stream,
            message: msg.to_string(),
        }
    }

    #[test]
    fn test_classify_error() {
        let c = Classifier::with_defaults();
        let e = make_entry("Connection error: ECONNREFUSED", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Error);
    }

    #[test]
    fn test_classify_warning() {
        let c = Classifier::with_defaults();
        let e = make_entry("Deprecated API called", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Warn);
    }

    #[test]
    fn test_classify_fatal() {
        let c = Classifier::with_defaults();
        let e = make_entry("FATAL: out of memory, panic", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Fatal);
    }

    #[test]
    fn test_classify_debug() {
        let c = Classifier::with_defaults();
        let e = make_entry("[debug] entering function foo", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Debug);
    }

    #[test]
    fn test_classify_info_default() {
        let c = Classifier::with_defaults();
        let e = make_entry("Server started on port 3000", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Info);
    }

    #[test]
    fn test_classify_stderr_floor() {
        let c = Classifier::with_defaults();
        // Normal message on stderr → at least Warn
        let e = make_entry("some output on stderr", 1, 1);
        assert_eq!(c.classify(&e), LogLevel::Warn);
    }

    #[test]
    fn test_classify_json_aware() {
        let c = Classifier::with_defaults();
        let e = make_entry(r#"{"level":"error","msg":"db connection lost"}"#, 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Error);
    }

    #[test]
    fn test_classify_respects_existing_level() {
        let c = Classifier::with_defaults();
        // Daemon already set level=3 (error)
        let e = make_entry("something", 3, 0);
        assert_eq!(c.classify(&e), LogLevel::Error);
    }

    #[test]
    fn test_custom_rule() {
        let mut c = Classifier::with_defaults();
        c.add_rule(r"SEGFAULT", LogLevel::Fatal, 15);
        let e = make_entry("SEGFAULT at 0x0000", 1, 0);
        assert_eq!(c.classify(&e), LogLevel::Fatal);
    }
}
