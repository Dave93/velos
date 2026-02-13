use std::collections::HashMap;

use crate::{LogLevel, ProcessedEntry};

/// Result of deduplication: a template with occurrence count.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DedupResult {
    pub template: String,
    pub count: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub level: LogLevel,
    pub sample: String,
}

/// Normalizes log messages by replacing variable parts with placeholders.
pub fn normalize(message: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static RE_UUID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
            .unwrap()
    });
    static RE_IP: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d+)?").unwrap());
    static RE_HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-fA-F]+").unwrap());
    static RE_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{2,}").unwrap());

    let s = RE_UUID.replace_all(message, "<UUID>");
    let s = RE_IP.replace_all(&s, "<IP>");
    let s = RE_HEX.replace_all(&s, "<HEX>");
    let s = RE_NUM.replace_all(&s, "<N>");
    s.into_owned()
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Deduplication engine with sliding time window.
pub struct DedupEngine {
    entries: HashMap<u64, DedupEntry>,
    window_ms: u64,
}

struct DedupEntry {
    template: String,
    count: u64,
    first_seen_ms: u64,
    last_seen_ms: u64,
    level: LogLevel,
    sample: String,
}

impl DedupEngine {
    pub fn new(window_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            window_ms: window_secs * 1000,
        }
    }

    /// Default: 60-second window.
    pub fn with_defaults() -> Self {
        Self::new(60)
    }

    /// Process entries and return deduplicated results.
    pub fn deduplicate(&mut self, entries: &[ProcessedEntry]) -> Vec<DedupResult> {
        self.entries.clear();

        for entry in entries {
            let normalized = normalize(&entry.message);
            let hash = hash_string(&normalized);

            if let Some(existing) = self.entries.get_mut(&hash) {
                // Check if within time window
                if entry.timestamp_ms.saturating_sub(existing.last_seen_ms) <= self.window_ms {
                    existing.count += 1;
                    existing.last_seen_ms = entry.timestamp_ms;
                    // Keep the highest severity level
                    if (entry.level as u8) > (existing.level as u8) {
                        existing.level = entry.level;
                    }
                    continue;
                }
            }

            self.entries.insert(
                hash,
                DedupEntry {
                    template: normalized,
                    count: 1,
                    first_seen_ms: entry.timestamp_ms,
                    last_seen_ms: entry.timestamp_ms,
                    level: entry.level,
                    sample: entry.message.clone(),
                },
            );
        }

        let mut results: Vec<DedupResult> = self
            .entries
            .values()
            .map(|e| DedupResult {
                template: e.template.clone(),
                count: e.count,
                first_seen_ms: e.first_seen_ms,
                last_seen_ms: e.last_seen_ms,
                level: e.level,
                sample: e.sample.clone(),
            })
            .collect();

        // Sort by count descending
        results.sort_by(|a, b| b.count.cmp(&a.count));
        results
    }
}

/// Format a dedup result for display.
pub fn format_dedup_result(r: &DedupResult) -> String {
    if r.count > 1 {
        let first = crate::format::format_timestamp_short(r.first_seen_ms);
        let last = crate::format::format_timestamp_short(r.last_seen_ms);
        format!(
            "{} (x{}, first: {}, last: {})",
            r.sample, r.count, first, last
        )
    } else {
        r.sample.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_ip() {
        assert_eq!(
            normalize("Connection to 192.168.1.5:5432 failed"),
            "Connection to <IP> failed"
        );
    }

    #[test]
    fn test_normalize_uuid() {
        assert_eq!(
            normalize("Request 550e8400-e29b-41d4-a716-446655440000 failed"),
            "Request <UUID> failed"
        );
    }

    #[test]
    fn test_normalize_numbers() {
        assert_eq!(
            normalize("Processed 1234 items in 567ms"),
            "Processed <N> items in <N>ms"
        );
    }

    #[test]
    fn test_normalize_hex() {
        assert_eq!(normalize("Segfault at 0xDEADBEEF"), "Segfault at <HEX>");
    }

    #[test]
    fn test_dedup_groups_similar() {
        let mut engine = DedupEngine::with_defaults();
        let entries = vec![
            ProcessedEntry {
                timestamp_ms: 1000,
                level: LogLevel::Error,
                stream: 0,
                message: "Connection to 192.168.1.1:5432 failed".into(),
            },
            ProcessedEntry {
                timestamp_ms: 2000,
                level: LogLevel::Error,
                stream: 0,
                message: "Connection to 10.0.0.5:5432 failed".into(),
            },
            ProcessedEntry {
                timestamp_ms: 3000,
                level: LogLevel::Error,
                stream: 0,
                message: "Connection to 172.16.0.1:5432 failed".into(),
            },
        ];
        let results = engine.deduplicate(&entries);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].count, 3);
        assert!(results[0].template.contains("<IP>"));
    }

    #[test]
    fn test_dedup_separates_different() {
        let mut engine = DedupEngine::with_defaults();
        let entries = vec![
            ProcessedEntry {
                timestamp_ms: 1000,
                level: LogLevel::Error,
                stream: 0,
                message: "Connection failed".into(),
            },
            ProcessedEntry {
                timestamp_ms: 2000,
                level: LogLevel::Info,
                stream: 0,
                message: "Server started".into(),
            },
        ];
        let results = engine.deduplicate(&entries);
        assert_eq!(results.len(), 2);
    }
}
