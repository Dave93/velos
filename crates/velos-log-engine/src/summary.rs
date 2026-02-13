use std::collections::HashMap;

use crate::anomaly::Anomaly;
use crate::pattern::DetectedPattern;
use crate::{LogLevel, ProcessedEntry};

/// Compact log summary for a process.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogSummary {
    pub process_name: String,
    pub period_start_ms: u64,
    pub period_end_ms: u64,
    pub total_lines: u64,
    pub by_level: HashMap<String, u64>,
    pub top_patterns: Vec<PatternSummary>,
    pub anomalies: Vec<Anomaly>,
    pub last_error: Option<String>,
    pub last_error_ms: Option<u64>,
    pub health_score: u8,
}

/// Compact pattern info for summary output.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternSummary {
    pub template: String,
    pub count: u32,
    pub trend: String,
}

impl From<&DetectedPattern> for PatternSummary {
    fn from(p: &DetectedPattern) -> Self {
        Self {
            template: p.template.clone(),
            count: p.frequency,
            trend: p.trend.as_str().to_string(),
        }
    }
}

/// Generate a full summary from processed entries and detector results.
pub fn generate_summary(
    process_name: &str,
    entries: &[ProcessedEntry],
    patterns: &[DetectedPattern],
    anomalies: &[Anomaly],
    restart_count: u32,
) -> LogSummary {
    let total_lines = entries.len() as u64;

    let mut by_level: HashMap<String, u64> = HashMap::new();
    let mut last_error: Option<String> = None;
    let mut last_error_ms: Option<u64> = None;
    let mut period_start = u64::MAX;
    let mut period_end = 0u64;

    for e in entries {
        *by_level.entry(e.level.as_str().to_string()).or_default() += 1;

        if e.timestamp_ms < period_start {
            period_start = e.timestamp_ms;
        }
        if e.timestamp_ms > period_end {
            period_end = e.timestamp_ms;
        }

        if matches!(e.level, LogLevel::Error | LogLevel::Fatal) {
            if last_error_ms.map_or(true, |ts| e.timestamp_ms > ts) {
                last_error = Some(e.message.clone());
                last_error_ms = Some(e.timestamp_ms);
            }
        }
    }

    if period_start == u64::MAX {
        period_start = 0;
    }

    let top_patterns: Vec<PatternSummary> = patterns.iter().take(5).map(|p| p.into()).collect();

    let health_score = compute_health_score(
        by_level.get("error").copied().unwrap_or(0)
            + by_level.get("fatal").copied().unwrap_or(0),
        anomalies.len() as u64,
        restart_count as u64,
    );

    LogSummary {
        process_name: process_name.to_string(),
        period_start_ms: period_start,
        period_end_ms: period_end,
        total_lines,
        by_level,
        top_patterns,
        anomalies: anomalies.to_vec(),
        last_error,
        last_error_ms,
        health_score,
    }
}

/// Health score: 100 - (errors * 5) - (anomalies * 10) - (restarts * 3), clamped to 0-100.
fn compute_health_score(error_count: u64, anomaly_count: u64, restart_count: u64) -> u8 {
    let penalty = (error_count * 5) + (anomaly_count * 10) + (restart_count * 3);
    100u8.saturating_sub(penalty.min(100) as u8)
}

/// Format summary for terminal display.
pub fn format_summary(s: &LogSummary) -> String {
    let mut out = String::new();

    let period = format_period(s.period_start_ms, s.period_end_ms);
    out.push_str(&format!(
        "Process: {} | Period: {} | Health: {}/100\n",
        s.process_name, period, s.health_score
    ));

    let errors = s.by_level.get("error").copied().unwrap_or(0)
        + s.by_level.get("fatal").copied().unwrap_or(0);
    let warnings = s.by_level.get("warn").copied().unwrap_or(0);
    out.push_str(&format!(
        "Lines: {} | Errors: {} | Warnings: {}\n",
        s.total_lines, errors, warnings
    ));

    if !s.top_patterns.is_empty() {
        out.push_str("Top patterns:\n");
        for (i, p) in s.top_patterns.iter().enumerate() {
            out.push_str(&format!(
                "  {}. \"{}\" (x{}, trend: {})\n",
                i + 1,
                truncate(&p.template, 60),
                p.count,
                p.trend
            ));
        }
    }

    if let Some(ref err) = s.last_error {
        let ago = if let Some(ts) = s.last_error_ms {
            let diff = s.period_end_ms.saturating_sub(ts);
            format_duration(diff)
        } else {
            "unknown".to_string()
        };
        out.push_str(&format!("Last error: \"{}\" ({} ago)\n", truncate(err, 60), ago));
    }

    for a in &s.anomalies {
        out.push_str(&format!(
            "Anomaly: {} {:.1}\u{03c3} above normal ({:.1}/min vs avg {:.1}/min)\n",
            a.metric, a.sigma, a.current_value, a.mean
        ));
    }

    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn format_period(start_ms: u64, end_ms: u64) -> String {
    let diff = end_ms.saturating_sub(start_ms);
    format!("last {}", format_duration(diff))
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_score_perfect() {
        assert_eq!(compute_health_score(0, 0, 0), 100);
    }

    #[test]
    fn test_health_score_errors() {
        // 10 errors * 5 = 50 penalty
        assert_eq!(compute_health_score(10, 0, 0), 50);
    }

    #[test]
    fn test_health_score_mixed() {
        // 5 errors * 5 + 2 anomalies * 10 + 3 restarts * 3 = 25 + 20 + 9 = 54
        assert_eq!(compute_health_score(5, 2, 3), 46);
    }

    #[test]
    fn test_health_score_clamped() {
        assert_eq!(compute_health_score(100, 100, 100), 0);
    }

    #[test]
    fn test_generate_summary() {
        let entries = vec![
            ProcessedEntry {
                timestamp_ms: 1000,
                level: LogLevel::Info,
                stream: 0,
                message: "ok".into(),
            },
            ProcessedEntry {
                timestamp_ms: 2000,
                level: LogLevel::Error,
                stream: 0,
                message: "db failed".into(),
            },
            ProcessedEntry {
                timestamp_ms: 3000,
                level: LogLevel::Info,
                stream: 0,
                message: "recovered".into(),
            },
        ];
        let summary = generate_summary("test-app", &entries, &[], &[], 0);
        assert_eq!(summary.process_name, "test-app");
        assert_eq!(summary.total_lines, 3);
        assert_eq!(summary.last_error.as_deref(), Some("db failed"));
        assert_eq!(summary.health_score, 95); // 1 error * 5 = 5 penalty
    }

    #[test]
    fn test_format_summary() {
        let summary = LogSummary {
            process_name: "api".into(),
            period_start_ms: 0,
            period_end_ms: 3600000,
            total_lines: 5000,
            by_level: [("info".into(), 4990), ("error".into(), 10)]
                .into_iter()
                .collect(),
            top_patterns: vec![],
            anomalies: vec![],
            last_error: Some("connection refused".into()),
            last_error_ms: Some(3500000),
            health_score: 50,
        };
        let output = format_summary(&summary);
        assert!(output.contains("Health: 50/100"));
        assert!(output.contains("Errors: 10"));
        assert!(output.contains("connection refused"));
    }
}
