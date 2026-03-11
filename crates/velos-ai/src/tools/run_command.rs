use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;

use super::{optional_str, optional_u64, required_str, safe_resolve, ToolExecutor};
use crate::types::ToolDefinition;

const DEFAULT_TIMEOUT_MS: u64 = 60_000; // 60 seconds
const MAX_OUTPUT: usize = 50_000; // 50KB

/// Commands that are blocked for safety.
const BLOCKED_PREFIXES: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "sudo ",
    "shutdown",
    "reboot",
    "mkfs",
    "dd if=",
    "chmod -R 777 /",
    ":(){",
    "fork bomb",
    "curl | sh",
    "curl | bash",
    "wget | sh",
    "wget | bash",
    "> /dev/sd",
];

pub struct RunCommand;

impl ToolExecutor for RunCommand {
    fn name(&self) -> &str {
        "run_command"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "run_command".into(),
            description: "Execute a shell command. Returns stdout and stderr. Has a timeout (default 60s). Some dangerous commands are blocked.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory (default: project root)"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout in milliseconds (default: 60000)"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn execute(&self, input: serde_json::Value, cwd: &Path) -> Result<String, String> {
        let cmd_str = required_str(&input, "command")?;
        let work_dir = match optional_str(&input, "cwd") {
            Some(p) => safe_resolve(&p, cwd)?,
            None => cwd.to_path_buf(),
        };
        let timeout =
            Duration::from_millis(optional_u64(&input, "timeout_ms").unwrap_or(DEFAULT_TIMEOUT_MS));

        // Safety check
        let cmd_lower = cmd_str.to_lowercase();
        for blocked in BLOCKED_PREFIXES {
            if cmd_lower.contains(blocked) {
                return Err(format!("blocked command: contains '{blocked}'"));
            }
        }

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&cmd_str)
            .current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn command: {e}"))?;

        let result = child
            .wait_timeout(timeout)
            .map_err(|e| format!("error waiting for command: {e}"))?;

        match result {
            Some(status) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                let mut output = String::new();
                output.push_str(&format!("Exit code: {}\n", status.code().unwrap_or(-1)));
                if !stdout.is_empty() {
                    output.push_str("\n--- stdout ---\n");
                    output.push_str(truncate(&stdout, MAX_OUTPUT));
                }
                if !stderr.is_empty() {
                    output.push_str("\n--- stderr ---\n");
                    output.push_str(truncate(&stderr, MAX_OUTPUT));
                }

                if status.success() {
                    Ok(output)
                } else {
                    // Non-zero exit is still returned as Ok (not an error) — AI needs to see the output
                    Ok(output)
                }
            }
            None => {
                // Timeout — kill the process
                let _ = child.kill();
                Err(format!("command timed out after {}ms", timeout.as_millis()))
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
