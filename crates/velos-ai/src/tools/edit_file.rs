use std::path::Path;
use serde_json::json;

use super::{ToolExecutor, safe_resolve, required_str};
use crate::types::ToolDefinition;

pub struct EditFile;

impl ToolExecutor for EditFile {
    fn name(&self) -> &str { "edit_file" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".into(),
            description: "Replace a specific text string in a file. The old_text must be unique in the file and match exactly (including whitespace/indentation).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "The exact text to find and replace (must be unique in the file)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The replacement text"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let path_str = required_str(&input, "path")?;
        let old_text = required_str(&input, "old_text")?;
        let new_text = required_str(&input, "new_text")?;
        let path = safe_resolve(&path_str, cwd)?;

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read file: {e}"))?;

        let count = content.matches(&old_text).count();
        if count == 0 {
            return Err(format!("old_text not found in {path_str}. Make sure it matches exactly, including whitespace."));
        }
        if count > 1 {
            return Err(format!("old_text found {count} times in {path_str}. It must be unique — provide more surrounding context."));
        }

        let new_content = content.replacen(&old_text, &new_text, 1);
        std::fs::write(&path, &new_content)
            .map_err(|e| format!("cannot write file: {e}"))?;

        Ok(format!("Edited {path_str}: replaced 1 occurrence."))
    }
}
