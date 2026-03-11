use serde_json::json;
use std::path::Path;

use super::{required_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

pub struct DeleteFile;

impl ToolExecutor for DeleteFile {
    fn name(&self) -> &str {
        "delete_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delete_file".into(),
            description: "Delete a file.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let path_str = required_str(&input, "path")?;
        let path = safe_resolve(&path_str, cwd)?;

        if !path.exists() {
            return Err(format!("{path_str} does not exist."));
        }
        if path.is_dir() {
            return Err(format!("{path_str} is a directory, not a file."));
        }

        std::fs::remove_file(&path).map_err(|e| format!("cannot delete file: {e}"))?;

        Ok(format!("Deleted {path_str}"))
    }
}
