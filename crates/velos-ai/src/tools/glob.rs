use serde_json::json;
use std::path::Path;

use super::{optional_str, required_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

const MAX_RESULTS: usize = 200;

pub struct GlobTool;

impl ToolExecutor for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".into(),
            description: "Find files matching a glob pattern. Returns file paths relative to the project root.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g. \"**/*.rs\", \"src/**/*.ts\", \"*.json\")"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (default: project root)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let pattern = required_str(&input, "pattern")?;
        let search_root = match optional_str(&input, "path") {
            Some(p) => safe_resolve(&p, cwd)?,
            None => cwd.to_path_buf(),
        };

        // Build full pattern
        let full_pattern = if pattern.starts_with('/') || pattern.starts_with("**") {
            format!("{}/{}", search_root.display(), pattern)
        } else {
            format!("{}/{}", search_root.display(), pattern)
        };

        let mut results = Vec::new();
        let entries =
            ::glob::glob(&full_pattern).map_err(|e| format!("invalid glob pattern: {e}"))?;

        for entry in entries {
            if results.len() >= MAX_RESULTS {
                break;
            }
            if let Ok(path) = entry {
                if path.is_file() {
                    let rel = path.strip_prefix(cwd).unwrap_or(&path);
                    results.push(rel.display().to_string());
                }
            }
        }

        if results.is_empty() {
            return Ok("No files found.".into());
        }

        let truncated = if results.len() >= MAX_RESULTS {
            format!("\n... (truncated at {MAX_RESULTS} results)")
        } else {
            String::new()
        };

        Ok(format!("{}{truncated}", results.join("\n")))
    }
}
