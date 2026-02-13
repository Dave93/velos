use crate::ProcessedEntry;

/// Format a log entry as structured JSON Line.
/// Output: `{"ts":1707734400000,"lvl":"info","pid":0,"msg":"Server started","src":"stdout"}`
pub fn format_structured(entry: &ProcessedEntry, pid: u32) -> String {
    let src = if entry.stream == 1 {
        "stderr"
    } else {
        "stdout"
    };
    serde_json::json!({
        "ts": entry.timestamp_ms,
        "lvl": entry.level.as_str(),
        "pid": pid,
        "msg": entry.message,
        "src": src,
    })
    .to_string()
}

/// Format a log entry as plain text.
/// Output: `[out|10:05:03] Server started on port 3000`
pub fn format_plain(entry: &ProcessedEntry) -> String {
    let stream_tag = if entry.stream == 1 { "err" } else { "out" };
    let time = format_timestamp(entry.timestamp_ms);
    format!("[{}|{}] {}", stream_tag, time, entry.message)
}

/// Format a log entry as plain text with level indicator.
/// Output: `[INFO|out|10:05:03] Server started on port 3000`
pub fn format_plain_with_level(entry: &ProcessedEntry) -> String {
    let stream_tag = if entry.stream == 1 { "err" } else { "out" };
    let time = format_timestamp(entry.timestamp_ms);
    let level = entry.level.as_str().to_uppercase();
    format!("[{}|{}|{}] {}", level, stream_tag, time, entry.message)
}

/// Short timestamp for dedup output (HH:MM:SS).
pub fn format_timestamp_short(ms: u64) -> String {
    format_timestamp(ms)
}

fn format_timestamp(ms: u64) -> String {
    let secs = (ms / 1000) % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogLevel;

    fn make_processed(msg: &str, level: LogLevel, stream: u8, ts: u64) -> ProcessedEntry {
        ProcessedEntry {
            timestamp_ms: ts,
            level,
            stream,
            message: msg.to_string(),
        }
    }

    #[test]
    fn test_format_structured() {
        let e = make_processed("Server started", LogLevel::Info, 0, 1707734400000);
        let json = format_structured(&e, 12345);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["lvl"], "info");
        assert_eq!(v["pid"], 12345);
        assert_eq!(v["msg"], "Server started");
        assert_eq!(v["src"], "stdout");
    }

    #[test]
    fn test_format_plain() {
        // 10:05:03 = 36303 seconds = 36303000 ms
        let e = make_processed("hello", LogLevel::Info, 0, 36303000);
        let s = format_plain(&e);
        assert_eq!(s, "[out|10:05:03] hello");
    }

    #[test]
    fn test_format_plain_stderr() {
        let e = make_processed("oops", LogLevel::Error, 1, 0);
        let s = format_plain(&e);
        assert!(s.starts_with("[err|"));
    }

    #[test]
    fn test_format_plain_with_level() {
        let e = make_processed("warning msg", LogLevel::Warn, 0, 0);
        let s = format_plain_with_level(&e);
        assert!(s.starts_with("[WARN|out|"));
    }
}
