use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::provider::AiProvider;
use crate::types::*;

// ---------------------------------------------------------------------------
// Crash context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashContext {
    pub process_name: String,
    pub exit_code: i32,
    pub hostname: String,
    pub timestamp: String,
    pub cwd: String,
    pub logs: Vec<String>,
    pub source_snippets: Vec<SourceSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSnippet {
    pub file: String,
    pub line: u32,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Crash record (persisted to ~/.velos/crashes/)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRecord {
    pub id: String,
    pub process_name: String,
    pub exit_code: i32,
    pub hostname: String,
    pub timestamp: String,
    pub cwd: String,
    pub logs: Vec<String>,
    pub analysis: String,
    pub status: CrashStatus,
    pub fix_result: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CrashStatus {
    Pending,
    Fixing,
    Fixed,
    Ignored,
    Failed,
}

impl CrashRecord {
    pub fn save(&self) -> Result<(), std::io::Error> {
        let dir = crashes_dir();
        std::fs::create_dir_all(&dir)?;
        let id = &self.id;
        let path = dir.join(format!("{id}.json"));
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn load(id: &str) -> Result<Self, std::io::Error> {
        let path = crashes_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(std::io::Error::other)
    }
}

fn crashes_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".velos")
        .join("crashes")
}

// ---------------------------------------------------------------------------
// Stack trace parsing
// ---------------------------------------------------------------------------

/// Extract file:line references from log lines.
pub fn extract_source_refs(logs: &[String]) -> Vec<(String, u32)> {
    let mut refs = Vec::new();
    let patterns = [
        // Node.js: "    at handler (/app/src/routes/api.ts:42:15)"
        regex::Regex::new(r"\(([^()]+):(\d+):\d+\)").unwrap(),
        // Node.js alt: "    at /app/src/routes/api.ts:42:15"
        regex::Regex::new(r"at\s+(/[^\s:]+):(\d+)").unwrap(),
        // Python: "  File \"/app/main.py\", line 42"
        regex::Regex::new(r#"File "([^"]+)", line (\d+)"#).unwrap(),
        // Rust: "   at src/main.rs:42:5"
        regex::Regex::new(r"at\s+(src/[^\s:]+):(\d+)").unwrap(),
        // Go: "main.go:42"
        regex::Regex::new(r"([a-zA-Z0-9_/]+\.go):(\d+)").unwrap(),
        // Generic: "file.ext:123" at line start or after whitespace
        regex::Regex::new(r"(?:^|\s)([a-zA-Z0-9_./-]+\.[a-zA-Z]+):(\d+)").unwrap(),
    ];

    for line in logs {
        for re in &patterns {
            for cap in re.captures_iter(line) {
                if let (Some(file), Some(line_num)) = (cap.get(1), cap.get(2)) {
                    if let Ok(n) = line_num.as_str().parse::<u32>() {
                        let f = file.as_str().to_string();
                        if !refs.iter().any(|(rf, rl)| rf == &f && *rl == n) {
                            refs.push((f, n));
                        }
                    }
                }
            }
        }
    }

    // Limit to most relevant (first 5)
    refs.truncate(5);
    refs
}

/// Read source code around a given line, returning context lines.
pub fn read_source_context(
    file: &str,
    line: u32,
    cwd: &Path,
    context: u32,
) -> Option<SourceSnippet> {
    let path = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        cwd.join(file)
    };

    let content = std::fs::read_to_string(&path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    let start = (line as usize).saturating_sub(context as usize + 1);
    let end = ((line as usize) + context as usize).min(lines.len());

    let snippet: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let num = start + i + 1;
            let marker = if num == line as usize { ">>>" } else { "   " };
            format!("{marker} {num:>4} | {l}")
        })
        .collect();

    Some(SourceSnippet {
        file: file.to_string(),
        line,
        content: snippet.join("\n"),
    })
}

// ---------------------------------------------------------------------------
// Analysis prompt
// ---------------------------------------------------------------------------

const ANALYSIS_SYSTEM: &str = "\
You are an expert software debugger. A process managed by Velos (a process manager) has crashed. \
Analyze the crash based on the logs and source code provided. \
\n\nProvide:\n\
1. Root cause — what exactly caused the crash\n\
2. The specific file and line where the error originates\n\
3. A brief explanation of why it happened\n\
4. A suggested fix (1-3 sentences)\n\n\
Be concise and direct. No boilerplate. Focus on actionable information.";

pub fn build_analysis_prompt(ctx: &CrashContext) -> String {
    let mut prompt = format!(
        "Process '{}' crashed with exit code {}.\n\n",
        ctx.process_name, ctx.exit_code
    );

    prompt.push_str("## Logs\n```\n");
    for line in &ctx.logs {
        prompt.push_str(line);
        prompt.push('\n');
    }
    prompt.push_str("```\n\n");

    if !ctx.source_snippets.is_empty() {
        prompt.push_str("## Relevant Source Code\n\n");
        for snippet in &ctx.source_snippets {
            prompt.push_str(&format!(
                "### {} (line {})\n```\n{}\n```\n\n",
                snippet.file, snippet.line, snippet.content
            ));
        }
    }

    if !ctx.cwd.is_empty() {
        prompt.push_str(&format!("Project directory: {}\n", ctx.cwd));
    }

    prompt
}

/// Run a simple analysis (no tools, single API call).
pub fn analyze(
    provider: &dyn AiProvider,
    ctx: &CrashContext,
) -> Result<String, crate::provider::AiError> {
    let prompt = build_analysis_prompt(ctx);
    let messages = vec![Message::user(prompt)];
    provider.chat(&messages, ANALYSIS_SYSTEM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_nodejs_stack_trace() {
        let logs = vec![
            "[ERR] TypeError: Cannot read property 'id' of undefined".into(),
            "    at handler (/app/src/routes/api.ts:42:15)".into(),
            "    at processTicksAndRejections (node:internal/process/task_queues:95:5)".into(),
        ];
        let refs = extract_source_refs(&logs);
        assert!(refs
            .iter()
            .any(|(f, l)| f == "/app/src/routes/api.ts" && *l == 42));
    }

    #[test]
    fn extract_python_stack_trace() {
        let logs = vec![
            "Traceback (most recent call last):".into(),
            "  File \"/app/main.py\", line 15, in <module>".into(),
            "    result = process(data)".into(),
        ];
        let refs = extract_source_refs(&logs);
        assert!(refs.iter().any(|(f, l)| f == "/app/main.py" && *l == 15));
    }

    #[test]
    fn extract_rust_stack_trace() {
        let logs = vec![
            "thread 'main' panicked at 'index out of bounds'".into(),
            "   at src/main.rs:42:5".into(),
        ];
        let refs = extract_source_refs(&logs);
        assert!(refs.iter().any(|(f, l)| f == "src/main.rs" && *l == 42));
    }

    #[test]
    fn crash_record_roundtrip() {
        let record = CrashRecord {
            id: "test-123".into(),
            process_name: "api".into(),
            exit_code: 1,
            hostname: "test".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            cwd: "/app".into(),
            logs: vec!["error".into()],
            analysis: "null pointer".into(),
            status: CrashStatus::Pending,
            fix_result: None,
            language: "en".into(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: CrashRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-123");
        assert_eq!(parsed.status, CrashStatus::Pending);
    }
}
