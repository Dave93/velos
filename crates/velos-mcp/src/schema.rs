use serde_json::Value;

pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        // Process tools
        ToolDefinition {
            name: "process_list",
            description: "List all managed processes with status, CPU, memory, uptime",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "process_start",
            description: "Start a new process",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "script": { "type": "string", "description": "Path to script or command to run" },
                    "name": { "type": "string", "description": "Process name (optional, defaults to script basename)" },
                    "cwd": { "type": "string", "description": "Working directory" },
                    "interpreter": { "type": "string", "description": "Interpreter (e.g. node, python3)" }
                },
                "required": ["script"]
            }),
        },
        ToolDefinition {
            name: "process_stop",
            description: "Stop a running process by name or ID",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" }
                },
                "required": ["name_or_id"]
            }),
        },
        ToolDefinition {
            name: "process_restart",
            description: "Restart a process by name or ID",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" }
                },
                "required": ["name_or_id"]
            }),
        },
        ToolDefinition {
            name: "process_delete",
            description: "Delete a stopped process by name or ID",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" }
                },
                "required": ["name_or_id"]
            }),
        },
        ToolDefinition {
            name: "process_info",
            description: "Get detailed information about a process (config, state, metrics)",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" }
                },
                "required": ["name_or_id"]
            }),
        },
        // Log tools
        ToolDefinition {
            name: "log_read",
            description: "Read last N log lines for a process, optionally filtered by level",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" },
                    "lines": { "type": "integer", "description": "Number of lines (default: 50)", "default": 50 },
                    "level": { "type": "string", "description": "Filter by level: debug,info,warn,error,fatal (comma-separated)" }
                },
                "required": ["name_or_id"]
            }),
        },
        ToolDefinition {
            name: "log_search",
            description: "Search logs by regex pattern with optional time range and level filter",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" },
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "since": { "type": "string", "description": "Start time (e.g. '1h', '30m', '2d')" },
                    "until": { "type": "string", "description": "End time" },
                    "level": { "type": "string", "description": "Filter by level (comma-separated)" }
                },
                "required": ["name_or_id", "pattern"]
            }),
        },
        ToolDefinition {
            name: "log_summary",
            description: "Get a compact log summary with health score, top patterns, anomalies (saves tokens)",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" },
                    "lines": { "type": "integer", "description": "Number of recent lines to analyze (default: 200)", "default": 200 }
                },
                "required": ["name_or_id"]
            }),
        },
        // Monitoring tools
        ToolDefinition {
            name: "health_check",
            description: "Check health of all processes (overall score + per-process details)",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "metrics_snapshot",
            description: "Get current metrics (memory, uptime, restarts) for one or all processes",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or ID (omit for all)" }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "config_get",
            description: "Get current configuration of a process",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" }
                },
                "required": ["name_or_id"]
            }),
        },
        ToolDefinition {
            name: "config_set",
            description: "Modify process configuration (env vars, restart policy, etc.)",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name_or_id": { "type": "string", "description": "Process name or numeric ID" },
                    "changes": { "type": "object", "description": "Key-value pairs to change" }
                },
                "required": ["name_or_id", "changes"]
            }),
        },
    ]
}
