use serde_json::json;
use std::path::Path;

use super::{optional_u64, required_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

const MAX_SIZE: u64 = 100 * 1024; // 100KB

pub struct ReadFile;

impl ToolExecutor for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read the contents of a file. Returns numbered lines.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-based, optional)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (optional)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let path_str = required_str(&input, "path")?;
        let path = safe_resolve(&path_str, cwd)?;

        let metadata = std::fs::metadata(&path).map_err(|e| format!("cannot read file: {e}"))?;

        if metadata.len() > MAX_SIZE {
            return Err(format!(
                "file too large: {} bytes (max {}). Use offset/limit to read a portion.",
                metadata.len(),
                MAX_SIZE
            ));
        }

        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read file: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        let offset = optional_u64(&input, "offset").unwrap_or(1).max(1) as usize;
        let limit = optional_u64(&input, "limit").map(|l| l as usize);

        let start = (offset - 1).min(lines.len());
        let end = match limit {
            Some(l) => (start + l).min(lines.len()),
            None => lines.len(),
        };

        let numbered: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4}  {}", start + i + 1, line))
            .collect();

        Ok(numbered.join("\n"))
    }
}
