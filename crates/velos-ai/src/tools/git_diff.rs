use serde_json::json;
use std::path::Path;
use std::process::Command;

use super::{optional_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

pub struct GitDiff;

impl ToolExecutor for GitDiff {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_diff".into(),
            description: "Show uncommitted git changes (staged and unstaged). Useful to review what has been modified.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory or file to diff (default: project root)"
                    }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let dir = match optional_str(&input, "path") {
            Some(p) => safe_resolve(&p, cwd)?,
            None => cwd.to_path_buf(),
        };

        // Get both staged and unstaged diff
        let output = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&dir)
            .output()
            .map_err(|e| format!("failed to run git diff: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(format!("git diff failed: {stderr}"));
        }

        if stdout.is_empty() {
            return Ok("No uncommitted changes.".into());
        }

        Ok(stdout.to_string())
    }
}
