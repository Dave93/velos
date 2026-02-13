use velos_core::VelosError;
use velos_log_engine::classifier::Classifier;
use velos_log_engine::dedup::DedupEngine;
use velos_log_engine::pattern::PatternDetector;
use velos_log_engine::summary;
use velos_log_engine::{format, LogLevel};

pub struct LogsArgs {
    pub name: String,
    pub lines: u32,
    pub json: bool,
    pub ai: bool,
    pub grep: Option<String>,
    pub level: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub dedupe: bool,
    pub summary: bool,
}

pub async fn run(args: LogsArgs) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let id = super::resolve_id(&mut client, &args.name).await?;

    let entries = client.logs(id, args.lines).await?;

    // Classify
    let classifier = Classifier::with_defaults();
    let mut processed = classifier.classify_batch(&entries);

    // Filter by level
    if let Some(ref levels) = args.level {
        let allowed: Vec<LogLevel> = levels
            .split(',')
            .filter_map(|l| match l.trim().to_lowercase().as_str() {
                "debug" => Some(LogLevel::Debug),
                "info" => Some(LogLevel::Info),
                "warn" | "warning" => Some(LogLevel::Warn),
                "error" | "err" => Some(LogLevel::Error),
                "fatal" => Some(LogLevel::Fatal),
                _ => None,
            })
            .collect();
        processed.retain(|e| allowed.contains(&e.level));
    }

    // Filter by grep pattern
    if let Some(ref pattern) = args.grep {
        let re = regex::Regex::new(pattern)
            .map_err(|e| VelosError::ProtocolError(format!("invalid grep pattern: {e}")))?;
        processed.retain(|e| re.is_match(&e.message));
    }

    // Filter by time range
    if let Some(ref since) = args.since {
        let since_ms = parse_time_spec(since)?;
        processed.retain(|e| e.timestamp_ms >= since_ms);
    }
    if let Some(ref until) = args.until {
        let until_ms = parse_time_spec(until)?;
        processed.retain(|e| e.timestamp_ms <= until_ms);
    }

    // Summary mode
    if args.summary {
        let detector = PatternDetector::with_defaults();
        let patterns = detector.detect(&processed);
        let log_summary = summary::generate_summary(&args.name, &processed, &patterns, &[], 0);

        if args.json || args.ai {
            println!(
                "{}",
                serde_json::to_string_pretty(&log_summary).unwrap_or_default()
            );
        } else {
            print!("{}", summary::format_summary(&log_summary));
        }
        return Ok(());
    }

    // Dedupe mode
    if args.dedupe {
        let mut engine = DedupEngine::with_defaults();
        let results = engine.deduplicate(&processed);

        if args.json || args.ai {
            println!(
                "{}",
                serde_json::to_string_pretty(&results).unwrap_or_default()
            );
        } else {
            if results.is_empty() {
                println!("[velos] No log entries for '{}'", args.name);
                return Ok(());
            }
            for r in &results {
                let level = r.level.as_str().to_uppercase();
                println!(
                    "[{}] {}",
                    level,
                    velos_log_engine::dedup::format_dedup_result(r)
                );
            }
        }
        return Ok(());
    }

    // Normal output
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&processed).unwrap_or_default()
        );
        return Ok(());
    }

    if args.ai {
        let compact: Vec<_> = processed
            .iter()
            .map(|e| {
                serde_json::json!({
                    "t": e.timestamp_ms,
                    "l": e.level.as_str(),
                    "m": e.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&compact).unwrap_or_default());
        return Ok(());
    }

    if processed.is_empty() {
        println!("[velos] No log entries for '{}'", args.name);
        return Ok(());
    }

    for entry in &processed {
        println!("{}", format::format_plain_with_level(entry));
    }

    Ok(())
}

/// Parse time spec: "1h", "30m", "2d", or ISO-like "2026-02-12 10:00".
fn parse_time_spec(spec: &str) -> Result<u64, VelosError> {
    let spec = spec.trim();

    // Relative: ends with h/m/s/d
    if let Some(num_str) = spec.strip_suffix('h') {
        let hours: u64 = num_str
            .parse()
            .map_err(|_| VelosError::ProtocolError(format!("invalid time: {spec}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        return Ok(now - hours * 3_600_000);
    }
    if let Some(num_str) = spec.strip_suffix('m') {
        let mins: u64 = num_str
            .parse()
            .map_err(|_| VelosError::ProtocolError(format!("invalid time: {spec}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        return Ok(now - mins * 60_000);
    }
    if let Some(num_str) = spec.strip_suffix('d') {
        let days: u64 = num_str
            .parse()
            .map_err(|_| VelosError::ProtocolError(format!("invalid time: {spec}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        return Ok(now - days * 86_400_000);
    }
    if let Some(num_str) = spec.strip_suffix('s') {
        let secs: u64 = num_str
            .parse()
            .map_err(|_| VelosError::ProtocolError(format!("invalid time: {spec}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        return Ok(now - secs * 1000);
    }

    // Absolute timestamp in ms
    if let Ok(ms) = spec.parse::<u64>() {
        return Ok(ms);
    }

    Err(VelosError::ProtocolError(format!(
        "unsupported time format: {spec} (use: 1h, 30m, 2d, or ms timestamp)"
    )))
}
