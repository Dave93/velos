use serde_json::json;
use std::path::Path;

use super::{optional_str, required_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

const MAX_MATCHES: usize = 100;

pub struct GrepTool;

impl ToolExecutor for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".into(),
            description: "Search file contents using a regex pattern. Returns matching lines with file path and line number.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search in (default: project root)"
                    },
                    "file_glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g. \"*.rs\", \"*.ts\")"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let pattern_str = required_str(&input, "pattern")?;
        let search_path = match optional_str(&input, "path") {
            Some(p) => safe_resolve(&p, cwd)?,
            None => cwd.to_path_buf(),
        };
        let file_glob = optional_str(&input, "file_glob");

        let re = regex::Regex::new(&pattern_str).map_err(|e| format!("invalid regex: {e}"))?;

        let mut matches = Vec::new();

        if search_path.is_file() {
            search_file(&search_path, &re, cwd, &mut matches);
        } else {
            let walker = walkdir::WalkDir::new(&search_path)
                .max_depth(10)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file());

            for entry in walker {
                if matches.len() >= MAX_MATCHES {
                    break;
                }

                let path = entry.path();

                // Skip binary/hidden/common non-text
                if should_skip(path) {
                    continue;
                }

                // Apply file glob filter
                if let Some(ref glob_pattern) = file_glob {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !simple_glob_match(glob_pattern, name) {
                        continue;
                    }
                }

                search_file(path, &re, cwd, &mut matches);
            }
        }

        if matches.is_empty() {
            return Ok("No matches found.".into());
        }

        let truncated = if matches.len() > MAX_MATCHES {
            matches.truncate(MAX_MATCHES);
            format!("\n... (truncated at {MAX_MATCHES} matches)")
        } else {
            String::new()
        };

        Ok(format!("{}{truncated}", matches.join("\n")))
    }
}

fn search_file(path: &Path, re: &regex::Regex, cwd: &Path, matches: &mut Vec<String>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return, // skip unreadable files
    };

    let rel = path.strip_prefix(cwd).unwrap_or(path);
    for (i, line) in content.lines().enumerate() {
        if matches.len() >= MAX_MATCHES {
            break;
        }
        if re.is_match(line) {
            matches.push(format!("{}:{}: {}", rel.display(), i + 1, line));
        }
    }
}

fn should_skip(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    // Skip common non-text directories
    if path_str.contains("/node_modules/")
        || path_str.contains("/.git/")
        || path_str.contains("/target/")
        || path_str.contains("/.zig-cache/")
        || path_str.contains("/dist/")
        || path_str.contains("/build/")
    {
        return true;
    }
    // Skip binary extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
                | "eot"
                | "zip"
                | "tar"
                | "gz"
                | "bin"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "a"
                | "o"
                | "pyc"
                | "class"
                | "wasm"
        )
    } else {
        false
    }
}

/// Simple glob matching for file names (supports * and ?).
fn simple_glob_match(pattern: &str, name: &str) -> bool {
    if pattern.starts_with("*.") {
        // Common case: "*.rs" -> check extension
        let ext = &pattern[2..];
        name.ends_with(&format!(".{ext}"))
    } else if pattern.contains('*') {
        // Basic wildcard: convert to regex
        let re_str = format!(
            "^{}$",
            pattern
                .replace('.', "\\.")
                .replace('*', ".*")
                .replace('?', ".")
        );
        regex::Regex::new(&re_str)
            .map(|re| re.is_match(name))
            .unwrap_or(false)
    } else {
        name == pattern
    }
}
