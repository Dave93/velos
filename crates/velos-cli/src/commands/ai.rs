use std::path::PathBuf;

use velos_ai::agent::Agent;
use velos_ai::analyzer::{self, CrashRecord, CrashStatus};
use velos_ai::i18n::I18n;
use velos_ai::provider::create_provider;
use velos_ai::tools::default_registry;
use velos_ai::types::AiConfig;
use velos_core::VelosError;

use super::config::{load_global_config, AiConfigToml};

// ---------------------------------------------------------------------------
// Fix
// ---------------------------------------------------------------------------

pub const FIX_SYSTEM: &str = "\
You are an expert software engineer. A process managed by Velos (a process manager) has crashed. \
Your job is to fix the bug by reading the code, understanding the error, and editing the files. \
\n\nYou have tools to read files, edit files, create files, search code, list directories, and run commands. \
Use them to:\n\
1. Understand the error from the logs and stack trace\n\
2. Find the relevant source code\n\
3. Fix the bug by editing the file(s)\n\
4. Verify your fix compiles/runs if possible\n\n\
Be precise and minimal. Only change what's necessary to fix the bug. \
Do not refactor unrelated code. After fixing, provide a brief summary of the changes.";

pub async fn run_fix(crash_id: String) -> Result<(), VelosError> {
    let config = load_global_config()?;
    let ai = require_ai_config(&config.ai)?;
    let language = config
        .notifications
        .as_ref()
        .and_then(|n| n.language.as_deref())
        .unwrap_or("en");
    let i18n = I18n::new(language);

    let mut record = CrashRecord::load(&crash_id)
        .map_err(|e| VelosError::ProtocolError(format!("{}: {e}", i18n.get("fix.no_crash_record"))))?;

    println!("{}", i18n.get("fix.started"));

    record.status = CrashStatus::Fixing;
    let _ = record.save();

    let ai_config = to_ai_config(&ai);
    let provider = create_provider(&ai_config)
        .map_err(|e| VelosError::ProtocolError(format!("AI provider: {e}")))?;

    let cwd = if record.cwd.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&record.cwd)
    };

    let prompt = build_fix_prompt(&record);

    let agent = Agent::new(
        provider,
        default_registry(),
        FIX_SYSTEM.to_string(),
        cwd,
        ai_config.max_iterations,
    );

    match agent.run(&prompt) {
        Ok(result) => {
            record.status = CrashStatus::Fixed;
            record.fix_result = Some(result.final_text.clone());
            let _ = record.save();

            println!("\n{}", i18n.get("fix.completed"));
            println!("  {} — {}", i18n.get("fix.iterations"), result.iterations);
            println!("  {} — {}", i18n.get("fix.tool_calls"), result.tool_calls);
            println!(
                "  {} — {}",
                i18n.get("fix.tokens"),
                result.total_usage.input_tokens + result.total_usage.output_tokens
            );
            println!("\n{}:\n{}", i18n.get("fix.changes_summary"), result.final_text);

            // Auto-restart the process after successful fix
            restart_process(&record.process_name).await;
        }
        Err(e) => {
            record.status = CrashStatus::Failed;
            record.fix_result = Some(format!("Error: {e}"));
            let _ = record.save();

            return Err(VelosError::ProtocolError(format!(
                "{}: {e}",
                i18n.get("fix.failed")
            )));
        }
    }

    Ok(())
}

async fn restart_process(process_name: &str) {
    // Suppress crash/error notifications for this restart
    set_suppress_notifications(process_name);

    let result = async {
        let mut client = crate::commands::connect().await?;
        let id = crate::commands::resolve_id(&mut client, process_name).await?;
        client.restart(id).await?;
        eprintln!("[velos-ai] restarted process '{process_name}' after fix");
        Ok::<(), VelosError>(())
    }
    .await;
    if let Err(e) = result {
        eprintln!("[velos-ai] failed to restart '{process_name}': {e}");
    }
}

/// Create a marker file to suppress the next crash/error notification for a process.
fn set_suppress_notifications(process_name: &str) {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("crashes");
    let _ = std::fs::create_dir_all(&dir);
    let marker = dir.join(format!(".suppress-{process_name}"));
    let _ = std::fs::write(&marker, "");
}

/// Check if notifications are suppressed for a process (and consume the marker).
pub fn take_suppress_notifications(process_name: &str) -> bool {
    let marker = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("crashes")
        .join(format!(".suppress-{process_name}"));
    if marker.exists() {
        let _ = std::fs::remove_file(&marker);
        true
    } else {
        false
    }
}

fn build_fix_prompt(record: &CrashRecord) -> String {
    let mut prompt = format!(
        "Process '{}' crashed with exit code {}.\n\n",
        record.process_name, record.exit_code
    );

    if !record.analysis.is_empty() {
        prompt.push_str("## Previous AI Analysis\n");
        prompt.push_str(&record.analysis);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## Logs\n```\n");
    for line in &record.logs {
        prompt.push_str(line);
        prompt.push('\n');
    }
    prompt.push_str("```\n\n");

    if !record.cwd.is_empty() {
        prompt.push_str(&format!("Project directory: {}\n\n", record.cwd));
    }

    prompt.push_str("Please fix this bug. Start by reading the relevant files, then make the necessary changes.");
    prompt
}

// ---------------------------------------------------------------------------
// Analyze
// ---------------------------------------------------------------------------

pub async fn run_analyze(crash_id: String) -> Result<(), VelosError> {
    let config = load_global_config()?;
    let ai = require_ai_config(&config.ai)?;

    let mut record = CrashRecord::load(&crash_id)
        .map_err(|e| VelosError::ProtocolError(format!("Crash record not found: {e}")))?;

    let ai_config = to_ai_config(&ai);
    let provider = create_provider(&ai_config)
        .map_err(|e| VelosError::ProtocolError(format!("AI provider: {e}")))?;

    let cwd = if record.cwd.is_empty() {
        String::new()
    } else {
        record.cwd.clone()
    };

    let source_refs = analyzer::extract_source_refs(&record.logs);
    let cwd_path = std::path::Path::new(&cwd);
    let source_snippets: Vec<_> = source_refs
        .iter()
        .filter_map(|(file, line)| analyzer::read_source_context(file, *line, cwd_path, 5))
        .collect();

    let ctx = analyzer::CrashContext {
        process_name: record.process_name.clone(),
        exit_code: record.exit_code,
        hostname: record.hostname.clone(),
        timestamp: record.timestamp.clone(),
        cwd,
        logs: record.logs.clone(),
        source_snippets,
    };

    println!("Analyzing crash {}...", crash_id);

    match analyzer::analyze(provider.as_ref(), &ctx) {
        Ok(analysis) => {
            record.analysis = analysis.clone();
            let _ = record.save();
            println!("\n{analysis}");
        }
        Err(e) => {
            return Err(VelosError::ProtocolError(format!("AI analysis failed: {e}")));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// List crashes
// ---------------------------------------------------------------------------

pub async fn run_list(json: bool) -> Result<(), VelosError> {
    let crashes_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("crashes");

    if !crashes_dir.exists() {
        if json {
            println!("[]");
        } else {
            println!("No crash records found.");
        }
        return Ok(());
    }

    let mut records = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&crashes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<CrashRecord>(&content) {
                        records.push(record);
                    }
                }
            }
        }
    }

    records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    if json {
        let json_str = serde_json::to_string_pretty(&records)
            .map_err(|e| VelosError::ProtocolError(e.to_string()))?;
        println!("{json_str}");
    } else if records.is_empty() {
        println!("No crash records found.");
    } else {
        println!(
            "{:<38} {:<15} {:<6} {:<10} {}",
            "ID", "PROCESS", "EXIT", "STATUS", "TIME"
        );
        for r in &records {
            let status = format!("{:?}", r.status).to_lowercase();
            println!(
                "{:<38} {:<15} {:<6} {:<10} {}",
                r.id, r.process_name, r.exit_code, status, r.timestamp
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Ignore
// ---------------------------------------------------------------------------

pub async fn run_ignore(crash_id: String) -> Result<(), VelosError> {
    let mut record = CrashRecord::load(&crash_id)
        .map_err(|e| VelosError::ProtocolError(format!("Crash record not found: {e}")))?;

    record.status = CrashStatus::Ignored;
    record
        .save()
        .map_err(|e| VelosError::ProtocolError(e.to_string()))?;

    println!("Crash {} marked as ignored.", crash_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_ai_config(ai: &Option<AiConfigToml>) -> Result<AiConfigToml, VelosError> {
    match ai {
        Some(ai) if !ai.provider.is_empty() && !ai.api_key.is_empty() => Ok(ai.clone()),
        _ => Err(VelosError::ProtocolError(
            "AI not configured. Set up with:\n  velos config set ai.provider anthropic\n  velos config set ai.api_key <key>"
                .into(),
        )),
    }
}

fn to_ai_config(ai: &AiConfigToml) -> AiConfig {
    AiConfig {
        provider: ai.provider.clone(),
        model: ai.model.clone(),
        api_key: ai.api_key.clone(),
        base_url: ai.base_url.clone(),
        max_iterations: ai.max_iterations,
        auto_analyze: ai.auto_analyze,
        auto_fix: ai.auto_fix,
    }
}
