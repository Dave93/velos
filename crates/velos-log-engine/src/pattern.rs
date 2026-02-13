use std::collections::HashMap;

use crate::{LogLevel, ProcessedEntry};

/// Detected trend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Trend {
    Rising,
    Stable,
    Declining,
}

impl Trend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Stable => "stable",
            Self::Declining => "declining",
        }
    }
}

/// A detected recurring log pattern.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedPattern {
    pub template: String,
    pub frequency: u32,
    pub level: LogLevel,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub trend: Trend,
}

/// Pattern detector: identifies recurring log message patterns.
pub struct PatternDetector {
    min_frequency: u32,
    time_window_ms: u64,
}

struct PatternBucket {
    template: String,
    count: u32,
    level: LogLevel,
    first_seen_ms: u64,
    last_seen_ms: u64,
    // Count in first half vs second half for trend detection
    first_half_count: u32,
    second_half_count: u32,
}

impl PatternDetector {
    pub fn new(min_frequency: u32, time_window_secs: u64) -> Self {
        Self {
            min_frequency,
            time_window_ms: time_window_secs * 1000,
        }
    }

    /// Default: min_frequency=5, time_window=5min.
    pub fn with_defaults() -> Self {
        Self::new(5, 300)
    }

    /// Detect patterns from a batch of entries.
    /// Returns patterns sorted by frequency descending.
    pub fn detect(&self, entries: &[ProcessedEntry]) -> Vec<DetectedPattern> {
        if entries.is_empty() {
            return Vec::new();
        }

        let now_ms = entries.last().map(|e| e.timestamp_ms).unwrap_or(0);
        let window_start = now_ms.saturating_sub(self.time_window_ms);
        let midpoint = window_start + self.time_window_ms / 2;

        let mut buckets: HashMap<u64, PatternBucket> = HashMap::new();

        for entry in entries {
            if entry.timestamp_ms < window_start {
                continue;
            }

            let normalized = crate::dedup::normalize(&entry.message);
            let hash = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                normalized.hash(&mut h);
                h.finish()
            };

            let bucket = buckets.entry(hash).or_insert_with(|| PatternBucket {
                template: normalized.clone(),
                count: 0,
                level: entry.level,
                first_seen_ms: entry.timestamp_ms,
                last_seen_ms: entry.timestamp_ms,
                first_half_count: 0,
                second_half_count: 0,
            });

            bucket.count += 1;
            bucket.last_seen_ms = entry.timestamp_ms;
            if (entry.level as u8) > (bucket.level as u8) {
                bucket.level = entry.level;
            }

            if entry.timestamp_ms < midpoint {
                bucket.first_half_count += 1;
            } else {
                bucket.second_half_count += 1;
            }
        }

        let mut patterns: Vec<DetectedPattern> = buckets
            .into_values()
            .filter(|b| b.count >= self.min_frequency)
            .map(|b| {
                let trend = detect_trend(b.first_half_count, b.second_half_count);
                DetectedPattern {
                    template: b.template,
                    frequency: b.count,
                    level: b.level,
                    first_seen_ms: b.first_seen_ms,
                    last_seen_ms: b.last_seen_ms,
                    trend,
                }
            })
            .collect();

        patterns.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        patterns
    }

    /// Detect top-N patterns.
    pub fn detect_top_n(&self, entries: &[ProcessedEntry], n: usize) -> Vec<DetectedPattern> {
        let mut patterns = self.detect(entries);
        patterns.truncate(n);
        patterns
    }
}

fn detect_trend(first_half: u32, second_half: u32) -> Trend {
    if first_half == 0 && second_half == 0 {
        return Trend::Stable;
    }
    let total = first_half + second_half;
    if total < 4 {
        return Trend::Stable;
    }
    let ratio = second_half as f64 / total as f64;
    if ratio > 0.65 {
        Trend::Rising
    } else if ratio < 0.35 {
        Trend::Declining
    } else {
        Trend::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(msg: &str, ts: u64) -> ProcessedEntry {
        ProcessedEntry {
            timestamp_ms: ts,
            level: LogLevel::Error,
            stream: 0,
            message: msg.to_string(),
        }
    }

    #[test]
    fn test_detect_patterns() {
        let detector = PatternDetector::new(3, 60);
        let entries: Vec<ProcessedEntry> = (0..10)
            .map(|i| make_entry(&format!("Connection to 10.0.0.{}:5432 failed", i), 1000 + i * 1000))
            .collect();
        let patterns = detector.detect(&entries);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].frequency, 10);
        assert!(patterns[0].template.contains("<IP>"));
    }

    #[test]
    fn test_min_frequency_filter() {
        let detector = PatternDetector::new(5, 60);
        let entries = vec![
            make_entry("rare event", 1000),
            make_entry("rare event", 2000),
        ];
        let patterns = detector.detect(&entries);
        assert!(patterns.is_empty()); // count=2 < min_frequency=5
    }

    #[test]
    fn test_trend_rising() {
        let detector = PatternDetector::new(2, 100);
        // All entries in second half → rising
        let entries: Vec<ProcessedEntry> = (0..10)
            .map(|i| make_entry("same error", 80000 + i * 1000))
            .collect();
        let patterns = detector.detect(&entries);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].trend, Trend::Rising);
    }

    #[test]
    fn test_trend_declining() {
        let detector = PatternDetector::new(2, 100);
        // All entries in first half → declining
        let entries: Vec<ProcessedEntry> = (0..10)
            .map(|i| make_entry("old error", 10000 + i * 1000))
            .collect();
        let patterns = detector.detect(&entries);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].trend, Trend::Declining);
    }

    #[test]
    fn test_top_n() {
        let detector = PatternDetector::new(1, 60);
        let mut entries = Vec::new();
        for i in 0..20 {
            entries.push(make_entry("frequent error", 1000 + i * 100));
        }
        for i in 0..5 {
            entries.push(make_entry("rare error", 1000 + i * 100));
        }
        let top = detector.detect_top_n(&entries, 1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].frequency, 20);
    }
}
