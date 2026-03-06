pub mod read_file;
pub mod edit_file;
pub mod create_file;
pub mod delete_file;
pub mod grep;
pub mod glob;
pub mod list_dir;
pub mod run_command;
pub mod git_diff;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::ToolDefinition;

// ---------------------------------------------------------------------------
// Tool trait
// ---------------------------------------------------------------------------

pub trait ToolExecutor: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    /// Execute the tool. Returns Ok(output) or Err(error_message).
    /// Errors are NOT fatal — they're sent back to the AI as tool_result with is_error=true.
    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn ToolExecutor>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    pub fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        cwd: &Path,
    ) -> Result<String, String> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(input, cwd),
            None => Err(format!("unknown tool: {name}")),
        }
    }
}

/// Create a registry with all built-in tools.
pub fn default_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(read_file::ReadFile));
    reg.register(Box::new(edit_file::EditFile));
    reg.register(Box::new(create_file::CreateFile));
    reg.register(Box::new(delete_file::DeleteFile));
    reg.register(Box::new(grep::GrepTool));
    reg.register(Box::new(glob::GlobTool));
    reg.register(Box::new(list_dir::ListDir));
    reg.register(Box::new(run_command::RunCommand));
    reg.register(Box::new(git_diff::GitDiff));
    reg
}

// ---------------------------------------------------------------------------
// Safety helpers
// ---------------------------------------------------------------------------

/// Resolve a path relative to cwd and ensure it doesn't escape the project directory.
pub fn safe_resolve(path_str: &str, cwd: &Path) -> Result<PathBuf, String> {
    let path = Path::new(path_str);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    // Canonicalize cwd (must exist)
    let canon_cwd = cwd
        .canonicalize()
        .map_err(|e| format!("cannot resolve cwd: {e}"))?;

    // For existing paths, canonicalize and check prefix
    if resolved.exists() {
        let canon = resolved
            .canonicalize()
            .map_err(|e| format!("cannot resolve path: {e}"))?;
        if !canon.starts_with(&canon_cwd) {
            return Err(format!(
                "path escapes project directory: {}",
                path_str
            ));
        }
        return Ok(canon);
    }

    // For non-existing paths (create_file), resolve parent and check
    if let Some(parent) = resolved.parent() {
        if parent.exists() {
            let canon_parent = parent
                .canonicalize()
                .map_err(|e| format!("cannot resolve parent: {e}"))?;
            if !canon_parent.starts_with(&canon_cwd) {
                return Err(format!(
                    "path escapes project directory: {}",
                    path_str
                ));
            }
            return Ok(canon_parent.join(resolved.file_name().unwrap_or_default()));
        }
    }

    // Fallback: join with cwd and do basic check
    let joined = canon_cwd.join(path_str);
    Ok(joined)
}

/// Extract a required string field from JSON input.
pub fn required_str(input: &serde_json::Value, field: &str) -> Result<String, String> {
    input[field]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing required field: {field}"))
}

/// Extract an optional string field from JSON input.
pub fn optional_str(input: &serde_json::Value, field: &str) -> Option<String> {
    input[field].as_str().map(|s| s.to_string())
}

/// Extract an optional u64 field.
pub fn optional_u64(input: &serde_json::Value, field: &str) -> Option<u64> {
    input[field].as_u64()
}
