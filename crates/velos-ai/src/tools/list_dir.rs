use std::path::Path;
use serde_json::json;

use super::{ToolExecutor, safe_resolve, optional_str, optional_u64};
use crate::types::ToolDefinition;

const MAX_ENTRIES: usize = 500;

pub struct ListDir;

impl ToolExecutor for ListDir {
    fn name(&self) -> &str { "list_dir" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_dir".into(),
            description: "List files and directories. Shows a tree-like structure with type indicators (dir/, file).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (default: project root)"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum recursion depth (default: 2)"
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
        let depth = optional_u64(&input, "depth").unwrap_or(2) as usize;

        if !dir.is_dir() {
            return Err(format!("{} is not a directory", dir.display()));
        }

        let mut entries = Vec::new();
        let walker = walkdir::WalkDir::new(&dir)
            .max_depth(depth)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in walker {
            if entries.len() >= MAX_ENTRIES {
                entries.push("... (truncated)".into());
                break;
            }
            let path = entry.path();
            // Skip hidden dirs content
            if path
                .components()
                .any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    s.starts_with('.') && s.len() > 1
                })
                && path != dir
            {
                continue;
            }
            // Skip common noise
            let path_str = path.to_string_lossy();
            if path_str.contains("/node_modules/")
                || path_str.contains("/target/")
                || path_str.contains("/.zig-cache/")
            {
                continue;
            }

            let rel = path.strip_prefix(&dir).unwrap_or(path);
            if rel.as_os_str().is_empty() {
                continue; // skip root itself
            }

            let indent = "  ".repeat(entry.depth().saturating_sub(1));
            let name = rel
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            if entry.file_type().is_dir() {
                entries.push(format!("{indent}{name}/"));
            } else {
                entries.push(format!("{indent}{name}"));
            }
        }

        if entries.is_empty() {
            return Ok("(empty directory)".into());
        }

        Ok(entries.join("\n"))
    }
}
