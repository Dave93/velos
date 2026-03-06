use std::path::Path;
use serde_json::json;

use super::{ToolExecutor, safe_resolve, required_str};
use crate::types::ToolDefinition;

pub struct CreateFile;

impl ToolExecutor for CreateFile {
    fn name(&self) -> &str { "create_file" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_file".into(),
            description: "Create a new file with the given content. Parent directories are created automatically. Fails if the file already exists — use edit_file instead.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let path_str = required_str(&input, "path")?;
        let content = required_str(&input, "content")?;
        let path = safe_resolve(&path_str, cwd)?;

        if path.exists() {
            return Err(format!("{path_str} already exists. Use edit_file to modify it."));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create directories: {e}"))?;
        }

        std::fs::write(&path, &content)
            .map_err(|e| format!("cannot write file: {e}"))?;

        Ok(format!("Created {path_str} ({} bytes)", content.len()))
    }
}
