# Velos MCP Server — Tool Reference

Velos includes a built-in MCP (Model Context Protocol) server that exposes 13 tools for managing processes, reading logs, and monitoring health — all optimized for token efficiency.

## Quick Start

### Running the MCP Server

```bash
velos mcp-server
```

The server reads JSON-RPC 2.0 messages from stdin and writes responses to stdout (stdio transport).

### Claude Desktop Configuration

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "velos": {
      "command": "velos",
      "args": ["mcp-server"]
    }
  }
}
```

### Claude Code Configuration

Add to `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "velos": {
      "command": "velos",
      "args": ["mcp-server"]
    }
  }
}
```

### Cursor Configuration

Add to Cursor Settings > MCP Servers:
- **Name**: velos
- **Command**: `velos mcp-server`

## Protocol

| Property | Value |
|----------|-------|
| Transport | stdio (stdin/stdout) |
| Protocol | JSON-RPC 2.0 |
| MCP Version | `2024-11-05` |
| Server Name | `velos` |
| Server Version | `0.1.0` |

Supported methods:
- `initialize` — handshake, returns server capabilities
- `tools/list` — list all available tools
- `tools/call` — execute a tool by name
- `ping` — health check (returns `{}`)

---

## Tools Reference

### Process Management

#### `process_list`

List all managed processes with status, CPU, memory, and uptime.

**Input**: none

**Output**: Array of process objects.

```json
[
  {
    "id": 0,
    "name": "api-server",
    "status": "running",
    "pid": 12345,
    "memory": 52428800,
    "uptime_ms": 3600000,
    "restarts": 0
  }
]
```

---

#### `process_start`

Start a new process.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `script` | string | yes | Path to script or command to run |
| `name` | string | no | Process name (defaults to script basename) |
| `cwd` | string | no | Working directory (defaults to current directory) |
| `interpreter` | string | no | Interpreter (e.g. `node`, `python3`) |

**Output**:

```json
{
  "id": 0,
  "name": "api-server",
  "status": "running"
}
```

Defaults applied: `autorestart: true`, `max_restarts: 15`, `min_uptime_ms: 1000`, `restart_delay_ms: 100`, `kill_timeout_ms: 5000`.

---

#### `process_stop`

Stop a running process by name or ID.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |

**Output**:

```json
{
  "success": true,
  "message": "stopped api-server"
}
```

---

#### `process_restart`

Restart a process by name or ID.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |

**Output**:

```json
{
  "success": true,
  "message": "restarted api-server"
}
```

---

#### `process_delete`

Delete a stopped process by name or ID.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |

**Output**:

```json
{
  "success": true,
  "message": "deleted api-server"
}
```

---

#### `process_info`

Get detailed information about a process (config, state, metrics).

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |

**Output**:

```json
{
  "id": 0,
  "name": "api-server",
  "pid": 12345,
  "status": "running",
  "memory": 52428800,
  "uptime_ms": 3600000,
  "restarts": 0,
  "script": "/app/server.js",
  "cwd": "/app",
  "interpreter": "node",
  "autorestart": true,
  "max_restarts": 15
}
```

---

### Log Analysis

#### `log_read`

Read last N log lines for a process, optionally filtered by level. Lines are classified by the Smart Log Engine (level detection, structured parsing).

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |
| `lines` | integer | no | Number of lines to read (default: 50) |
| `level` | string | no | Filter by level: `debug`, `info`, `warn`, `error`, `fatal` (comma-separated) |

**Output**: Array of log entries with compact keys.

```json
[
  { "t": 1700000000000, "l": "info", "m": "Server listening on :3000" },
  { "t": 1700000001000, "l": "error", "m": "Connection refused to database" }
]
```

---

#### `log_search`

Search logs by regex pattern with optional level filter. Scans the last 500 log lines.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |
| `pattern` | string | yes | Regex pattern to search for |
| `since` | string | no | Start time (e.g. `1h`, `30m`, `2d`) |
| `until` | string | no | End time |
| `level` | string | no | Filter by level (comma-separated) |

**Output**: Same format as `log_read` — array of matching entries.

```json
[
  { "t": 1700000001000, "l": "error", "m": "Connection refused to database" },
  { "t": 1700000005000, "l": "error", "m": "Connection refused to cache" }
]
```

---

#### `log_summary`

Get a compact log summary with health score, top patterns, and anomalies. This is the most token-efficient way to understand log state — it replaces thousands of log lines with a structured overview.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |
| `lines` | integer | no | Number of recent lines to analyze (default: 200) |

**Output**:

```json
{
  "process_name": "api-server",
  "period_start_ms": 1700000000000,
  "period_end_ms": 1700003600000,
  "total_lines": 200,
  "by_level": {
    "info": 185,
    "warn": 5,
    "error": 10
  },
  "top_patterns": [
    { "template": "GET /api/users 200 *ms", "count": 120, "trend": "stable" },
    { "template": "Connection refused to *", "count": 8, "trend": "increasing" }
  ],
  "anomalies": [],
  "last_error": "Connection refused to database",
  "last_error_ms": 1700003500000,
  "health_score": 50
}
```

**Health score formula**: `100 - (errors * 5) - (anomalies * 10) - (restarts * 3)`, clamped to 0–100.

---

### Monitoring & Configuration

#### `health_check`

Check health of all processes with an overall score and per-process details.

**Input**: none

**Output**:

```json
{
  "overall_score": 85,
  "process_count": 3,
  "processes": [
    {
      "name": "api-server",
      "score": 100,
      "status": "running",
      "issues": []
    },
    {
      "name": "worker",
      "score": 85,
      "status": "running",
      "issues": ["5 restarts"]
    },
    {
      "name": "cron-job",
      "score": 50,
      "status": "stopped",
      "issues": ["status: stopped"]
    }
  ]
}
```

**Scoring**: starts at 100, -50 for non-running status, -3 per restart (max -30). Overall score = minimum across all processes.

---

#### `metrics_snapshot`

Get current metrics for one or all processes.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | no | Process name or ID (omit for all processes) |

**Output** (single process):

```json
{
  "name": "api-server",
  "memory": 52428800,
  "uptime_ms": 3600000,
  "restarts": 0,
  "status": "running"
}
```

**Output** (all processes): Array of the above objects.

---

#### `config_get`

Get current configuration of a process.

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |

**Output**:

```json
{
  "name": "api-server",
  "script": "/app/server.js",
  "cwd": "/app",
  "interpreter": "node",
  "kill_timeout_ms": 5000,
  "autorestart": true,
  "max_restarts": 15,
  "min_uptime_ms": 1000,
  "restart_delay_ms": 100,
  "exp_backoff": false,
  "max_memory_restart": 0,
  "watch": false,
  "cron_restart": "",
  "wait_ready": false,
  "shutdown_with_message": false
}
```

---

#### `config_set`

> **Not yet implemented.** Requires daemon support. Currently returns an error.

Modify process configuration (env vars, restart policy, etc.).

**Input**:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name_or_id` | string | yes | Process name or numeric ID |
| `changes` | object | yes | Key-value pairs to change |

---

## Token Economy

Velos MCP is designed for token efficiency — every response is compact by default:

| Tool | Typical Response | Equivalent Manual Approach |
|------|-----------------|---------------------------|
| `process_list` | ~89 tokens | ~2,847 tokens (PM2 JSON output) |
| `log_summary` | ~150 tokens | ~50,000 lines of raw logs |
| `health_check` | ~100 tokens | Multiple commands + manual analysis |

Key design choices:
- Compact JSON keys in log entries (`t`, `l`, `m` instead of `timestamp`, `level`, `message`)
- `log_summary` uses algorithmic pattern detection and anomaly analysis — **zero LLM cost**
- `health_check` provides a single numeric score, eliminating the need to inspect each process individually

---

## Examples

### Example: List all processes

**Request**:
```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"process_list","arguments":{}}}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{
      "type": "text",
      "text": "[{\"id\":0,\"name\":\"api\",\"status\":\"running\",\"pid\":1234,\"memory\":52428800,\"uptime_ms\":3600000,\"restarts\":0}]"
    }]
  }
}
```

### Example: Start a process

**Request**:
```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"process_start","arguments":{"script":"server.js","name":"api","interpreter":"node"}}}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"id\":0,\"name\":\"api\",\"status\":\"running\"}"
    }]
  }
}
```

### Example: Diagnose an error

A typical diagnostic flow using MCP:

1. **Check overall health**:
   ```
   tools/call → health_check
   → overall_score: 50, "worker" has issues: ["status: stopped", "3 restarts"]
   ```

2. **Get log summary for the failing process**:
   ```
   tools/call → log_summary { name_or_id: "worker" }
   → health_score: 35, top pattern: "ECONNREFUSED 127.0.0.1:5432" (x12, increasing)
   ```

3. **Search for specific errors**:
   ```
   tools/call → log_search { name_or_id: "worker", pattern: "ECONNREFUSED", level: "error" }
   → 12 matching entries with timestamps
   ```

4. **Restart after fixing the root cause**:
   ```
   tools/call → process_restart { name_or_id: "worker" }
   → { success: true, message: "restarted worker" }
   ```

### Example: Monitor resource usage

```
tools/call → metrics_snapshot {}
→ [
    { name: "api", memory: 52428800, uptime_ms: 86400000, restarts: 0, status: "running" },
    { name: "worker", memory: 104857600, uptime_ms: 3600000, restarts: 3, status: "running" }
  ]
```

---

## Error Handling

When a tool encounters an error, the response includes `isError: true`:

```json
{
  "content": [{
    "type": "text",
    "text": "Error: process not found: unknown-app"
  }],
  "isError": true
}
```

Common errors:
- `process not found: <name>` — no process with that name or ID
- `missing '<field>' argument` — required parameter not provided
- `invalid regex pattern: <details>` — bad regex in `log_search`
- `config_set not yet implemented` — feature pending daemon support
