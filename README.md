<p align="center">
  <h1 align="center">Velos</h1>
  <p align="center"><strong>High-performance AI-friendly process manager</strong></p>
  <p align="center">Zig core + Rust shell. Next-gen PM2 alternative with native MCP server and zero-LLM smart log analysis.</p>
</p>

<p align="center">
  <a href="https://github.com/Dave93/velos/actions"><img src="https://img.shields.io/github/actions/workflow/status/Dave93/velos/ci.yml?branch=main&label=CI" alt="CI"></a>
  <a href="https://github.com/Dave93/velos/releases"><img src="https://img.shields.io/github/v/release/Dave93/velos?label=version" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License"></a>
  <a href="https://crates.io/crates/velos"><img src="https://img.shields.io/crates/v/velos" alt="crates.io"></a>
</p>

---

## Why Velos?

| Feature | Velos | PM2 | Supervisor |
|---------|-------|-----|------------|
| Language | Zig + Rust | Node.js | Python |
| Daemon memory | ~2 MB | ~60 MB | ~30 MB |
| MCP server (AI agents) | Built-in (13 tools) | - | - |
| Smart log analysis | Algorithmic (zero LLM cost) | - | - |
| AI crash analysis + auto-fix | Built-in (Anthropic/OpenAI) | - | - |
| Cluster mode | `velos start -i max` | `pm2 start -i max` | - |
| Prometheus metrics | Built-in | pm2-prometheus-exporter | - |
| REST API + WebSocket | Built-in | pm2-api | - |
| TUI dashboard | `velos monit` | `pm2 monit` | - |
| Config format | TOML | JSON/JS/YAML | INI |
| Watch mode | kqueue/inotify | chokidar (Node.js) | - |
| Shell completions | bash, zsh, fish | - | - |

## Quick Start

### Install

```bash
# One-line installer (macOS / Linux)
curl -fsSL https://releases.velospm.dev/install.sh | bash

# macOS (Homebrew)
brew install Dave93/tap/velos

# Debian / Ubuntu
curl -fsSL https://releases.velospm.dev/setup-apt.sh | sudo bash

# RHEL / Fedora / CentOS
curl -fsSL https://releases.velospm.dev/setup-yum.sh | sudo bash

# From source (requires Zig 0.15+ and Rust 1.75+)
git clone https://github.com/Dave93/velos.git
cd velos
make build
```

### Usage

```bash
# Terminal 1: start the daemon
velos daemon

# Terminal 2: manage processes
velos start server.js --name api
velos start worker.py --name bg -i 4     # cluster mode: 4 instances
velos list                                # show all processes
velos logs api --summary                  # smart log summary
velos monit                               # TUI dashboard
```

### Shell Completions

```bash
# Bash
velos completions bash > ~/.bash_completion.d/velos
# Zsh
velos completions zsh > ~/.zfunc/_velos
# Fish
velos completions fish > ~/.config/fish/completions/velos.fish
```

---

## Features

### Process Management
- **Start/stop/restart/reload** with graceful shutdown (SIGTERM -> SIGKILL)
- **Autorestart** with crash loop detection (max_restarts, min_uptime, exp_backoff)
- **Cluster mode** — multi-instance with `velos start -i N` or `-i max`
- **Watch mode** — auto-restart on file changes (kqueue/inotify)
- **Memory limits** — restart when RSS exceeds threshold (`--max-memory 150M`)
- **Cron restart** — periodic restart on schedule (`--cron-restart "0 3 * * *"`)
- **Ready signal** — process reports readiness via IPC (`--wait-ready`)
- **Graceful shutdown** — JSON message via IPC instead of SIGTERM (`--shutdown-with-message`)
- **State persistence** — save/resurrect process list across daemon restarts

### Smart Log Engine (Zero LLM Cost)
All "smart" features are algorithmic — regex, statistics, heuristics. No LLM API calls.

- **Auto-classifier** — detect log levels (regex + JSON-aware)
- **Deduplication** — collapse repeated messages with count and time range
- **Pattern detection** — frequency analysis with trend (rising/stable/declining)
- **Anomaly detection** — sliding window, mean + stddev, 2σ/3σ thresholds
- **Summary** — health score (0-100), top patterns, anomalies, last error

```bash
velos logs api --summary
velos logs api --level error --grep "timeout" --dedupe
```

### MCP Server (AI Agent Integration)
Built-in [Model Context Protocol](https://modelcontextprotocol.io/) server with 13 tools for AI agents. Supports **stdio** (local) and **Streamable HTTP** (remote) transports.

#### Local (stdio) — AI client and Velos on the same machine

<details>
<summary><b>Claude Code</b> — <code>.mcp.json</code> in project root</summary>

```json
{
  "mcpServers": {
    "velos": { "command": "velos", "args": ["mcp-server"] }
  }
}
```
</details>

<details>
<summary><b>Claude Desktop</b> — <code>~/Library/Application Support/Claude/claude_desktop_config.json</code></summary>

```json
{
  "mcpServers": {
    "velos": { "command": "/Users/you/.velos/bin/velos", "args": ["mcp-server"] }
  }
}
```
</details>

<details>
<summary><b>Cursor</b> — <code>.cursor/mcp.json</code></summary>

```json
{
  "mcpServers": {
    "velos": { "command": "velos", "args": ["mcp-server"] }
  }
}
```
</details>

<details>
<summary><b>OpenAI Codex</b> — <code>~/.codex/config.toml</code></summary>

```toml
[mcp_servers.velos]
command = "velos"
args = ["mcp-server"]
```

Or via CLI: `codex mcp add velos -- velos mcp-server`
</details>

<details>
<summary><b>Gemini CLI</b> — <code>~/.gemini/settings.json</code></summary>

```json
{
  "mcpServers": {
    "velos": { "command": "velos", "args": ["mcp-server"] }
  }
}
```
</details>

#### Remote (HTTP) — Velos on VPS, AI client on your laptop

Start MCP server on your VPS:
```bash
velos mcp-server --port 8080
```

Then configure your AI client with the remote URL:

<details>
<summary><b>Claude Code / Claude Desktop / Cursor</b></summary>

```json
{
  "mcpServers": {
    "velos": { "type": "url", "url": "http://your-vps:8080/mcp" }
  }
}
```
</details>

<details>
<summary><b>OpenAI Codex</b> — <code>~/.codex/config.toml</code></summary>

```toml
[mcp_servers.velos]
url = "http://your-vps:8080/mcp"
```
</details>

<details>
<summary><b>Gemini CLI</b> — <code>~/.gemini/settings.json</code></summary>

```json
{
  "mcpServers": {
    "velos": { "url": "http://your-vps:8080/mcp" }
  }
}
```
</details>

> **Tip:** For production, use HTTPS via a reverse proxy (nginx/caddy) or SSH tunnel:
> ```bash
> ssh -L 8080:localhost:8080 user@your-vps
> ```

#### Available Tools

| Tool | Description |
|------|-------------|
| `process_list` | List processes with status, memory, uptime |
| `process_start` | Start a new process |
| `process_stop` | Stop by name or ID |
| `process_restart` | Restart by name or ID |
| `process_delete` | Delete a stopped process |
| `process_info` | Detailed info (config + state + metrics) |
| `log_read` | Last N lines with level filter |
| `log_search` | Regex search with time range |
| `log_summary` | Health score, patterns, anomalies (~150 tokens vs 50K lines) |
| `health_check` | Overall + per-process health score |
| `metrics_snapshot` | Current CPU, RAM, restarts, uptime |
| `config_get` | Process configuration |
| `config_set` | Modify config *(planned)* |

Full reference: [docs/mcp-tools.md](docs/mcp-tools.md)

### AI Output Mode (`--ai`)

The `--ai` flag outputs compact JSON with abbreviated keys, designed to minimize token usage when piping Velos output to LLMs or AI agents.

```bash
# Standard output
velos list
#  ID  Name  PID    Status   Memory    Uptime  Restarts
#  0   api   1234   online   45.2 MB   2h 30m  0

# JSON output (full keys)
velos list --json
# [{"id": 0, "name": "api", "pid": 1234, "status": "online", "memory_bytes": 47395840, ...}]

# AI output (abbreviated keys, ~40% fewer tokens)
velos list --ai
# [{"n":"api","i":0,"s":"online","m":47395840,"u":9000000,"r":0,"p":1234}]
```

**Key abbreviations:** `n`=name, `i`=id, `s`=status, `p`=pid, `m`=memory (bytes), `u`=uptime (ms), `r`=restarts, `c`=cpu (%), `t`=timestamp, `l`=level

Supported commands: `velos list --ai`, `velos info <name> --ai`, `velos logs <name> --ai`

### AI Crash Analysis & Auto-Fix
When a process crashes, Velos can analyze the error with AI and auto-fix the bug. Supports **Anthropic** (Claude) and **OpenAI-compatible** providers (OpenAI, OpenRouter, Groq, Ollama, xAI).

```bash
# Configure AI provider
velos config set ai.provider anthropic
velos config set ai.api_key sk-ant-...
velos config set ai.model claude-sonnet-4-20250514

# Optional: enable auto-analysis on crash
velos config set ai.auto_analyze true

# Configure Telegram for crash notifications
velos config set telegram.bot_token 123456:ABC...
velos config set telegram.chat_id -100123456789
velos config set notifications.language ru    # en or ru
```

On crash: logs are collected, stack traces parsed, source code extracted, AI analysis runs, and a Telegram notification is sent with **Fix** / **Ignore** inline buttons.

```bash
# Manual commands
velos ai list                    # list crash records
velos ai analyze <crash-id>     # re-analyze a crash
velos ai fix <crash-id>         # run AI agent to auto-fix
velos ai ignore <crash-id>      # mark as ignored
```

The AI fix agent has 9 tools: read/edit/create/delete files, grep, glob, list directories, run commands, and git diff — all sandboxed to the project directory.

### Monitoring & Metrics
- **TUI dashboard** (`velos monit`) — real-time process table, memory sparkline, live logs
- **Prometheus endpoint** (`velos metrics -p 9615`) — scrape at `/metrics`
- **OpenTelemetry** — OTLP export (`--otel-endpoint`)
- **REST API** (`velos api -p 3100`) — JSON API + WebSocket real-time updates

---

## CLI Reference

All commands support `--json` for machine-readable output.

| Command | Description |
|---------|-------------|
| `velos daemon` | Run daemon in foreground |
| `velos start <script>` | Start a process (or `--config velos.toml`) |
| `velos stop <name\|id>` | Stop a process |
| `velos restart <name\|id\|all>` | Restart process(es) |
| `velos reload <name\|id\|all>` | Graceful reload |
| `velos list` | List all processes (alias: `ls`) |
| `velos info <name\|id>` | Detailed process info |
| `velos logs <name>` | Show logs with smart analysis |
| `velos delete <name\|id>` | Delete a process |
| `velos save` | Save process list to state file |
| `velos resurrect` | Restore saved processes |
| `velos flush [name\|id]` | Flush log files |
| `velos scale <name> <count>` | Scale cluster instances (+N, -N, max) |
| `velos monit` | TUI monitoring dashboard |
| `velos metrics` | Start Prometheus exporter |
| `velos api` | Start REST API + WebSocket server |
| `velos mcp-server` | Start MCP server (stdio or `--port` for HTTP) |
| `velos startup` | Generate init system script |
| `velos unstartup` | Remove init system script |
| `velos completions <shell>` | Generate shell completions |
| `velos config set <key> <val>` | Set global config value |
| `velos config get [key]` | Show config value(s) |
| `velos ai list` | List crash records |
| `velos ai fix <id>` | Auto-fix crash with AI agent |
| `velos ai analyze <id>` | Re-analyze crash with AI |
| `velos ai ignore <id>` | Mark crash as ignored |
| `velos ping` | Check daemon connectivity |

### Key Flags

```bash
# Start options
velos start app.js --name api --watch --max-memory 256M
velos start app.js -i 4                    # cluster: 4 instances
velos start app.js -i max                  # cluster: CPU count instances
velos start app.js --cron-restart "0 3 * * *"
velos start app.js --wait-ready --shutdown-with-message
velos start --config velos.toml

# Log options
velos logs api -l 200                      # last 200 lines
velos logs api --level error,warn          # filter by level
velos logs api --grep "timeout"            # regex filter
velos logs api --since "1h" --dedupe       # last hour, deduplicated
velos logs api --summary                   # health score + patterns

# Output modes
velos list --json                          # full JSON
velos list --ai                            # compact JSON for LLM
```

---

## Configuration (TOML)

```toml
[apps.api]
script = "server.js"
cwd = "/app"
interpreter = "node"
autorestart = true
max_restarts = 15
min_uptime = 1000
kill_timeout = 5000
max_memory_restart = "150M"

# File watching
watch = true
watch_paths = ["src/", "config/"]
watch_ignore = ["node_modules", ".git", "*.log"]
watch_delay = 1000

# Environment variables
[apps.api.env]
NODE_ENV = "production"
PORT = "3000"

# Profile-specific env (--env production)
[apps.api.env_production]
DATABASE_URL = "postgres://prod:5432/db"

[apps.worker]
script = "worker.py"
interpreter = "python3"
autorestart = true
max_memory_restart = "256M"
```

```bash
velos start --config velos.toml
velos start --config velos.toml --env production
```

Full example: [`config/velos.example.toml`](config/velos.example.toml)

---

## Architecture

```
┌──────────────┐     Unix socket     ┌──────────────────────┐
│  velos CLI   │ <──── IPC ────────> │   Zig Daemon Core    │
│  (Rust)      │   binary protocol   │  fork/exec, kqueue,  │
│  clap, tokio │                     │  CPU/RAM monitoring,  │
└──────────────┘                     │  log collector,       │
                                     │  file watcher,        │
                                     │  cron scheduler       │
                                     └──────────────────────┘
```

**Zig core** (`zig/src/`): daemon, fork/exec, IPC server, event loop (kqueue/epoll), CPU/RAM monitoring via syscalls, log collector, ring buffer, file watcher, cron parser, IPC channel (socketpair).

**Rust shell** (`crates/`): CLI (clap), IPC client, TOML config, Smart Log Engine, MCP Server (JSON-RPC stdio), Prometheus/OpenTelemetry, REST API (axum), TUI (ratatui).

**Bridge**: Zig compiles to `libvelos_core.a` (static library, C ABI) -> Rust links via FFI.

| Crate | Role |
|-------|------|
| `velos-ffi` | FFI bindings to Zig (extern "C", safe wrappers) |
| `velos-core` | Shared types: ProcessConfig, ProcessState, IPC protocol, errors |
| `velos-client` | IPC client (Unix socket -> daemon) |
| `velos-config` | TOML parsing and validation |
| `velos-log-engine` | Smart Logs: classifier, dedup, patterns, anomaly, summary |
| `velos-mcp` | MCP Server (stdio, JSON-RPC, 13 tools) |
| `velos-metrics` | Prometheus exporter, OpenTelemetry |
| `velos-api` | REST API + WebSocket (axum) |
| `velos-ai` | AI crash analysis, agent with tools, multi-provider |
| `velos-cli` | CLI binary (clap, ratatui TUI) |

**IPC protocol**: binary, 7-byte header (magic `0xVE10` + version + length LE u32) + MessagePack payload. Unix socket at `~/.velos/velos.sock`.

Full architecture: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

---

## Runtime Directory

```
~/.velos/
├── velos.sock          # IPC Unix socket
├── velos.pid           # Daemon PID file
├── state.bin           # Saved process state (velos save / auto-save)
└── logs/               # Process log files
    ├── api-out.log     # stdout
    └── api-err.log     # stderr
```

---

## Building from Source

### Requirements
- **Zig** 0.15+
- **Rust** 1.75+ (with cargo)
- **macOS** or **Linux**

### Build

```bash
make dev          # Debug: Zig + Rust (fast iteration)
make build        # Release: Zig (ReleaseFast) + Rust (release)
make test         # All tests: Zig unit + Rust unit
make clean        # Clean build artifacts
```

Pipeline: `zig build` -> `libvelos_core.a` -> `cargo build` -> `target/release/velos`

### Tests

```bash
# Zig unit tests
cd zig && zig build test

# Rust unit tests
cargo test --workspace

# Integration tests (35 tests, full lifecycle)
bash tests/integration_lifecycle.sh
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [CHANGELOG.md](CHANGELOG.md) | Release notes |
| [docs/CONCEPT.md](docs/CONCEPT.md) | Vision, features, competitor comparison |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Full architecture (1200+ lines) |
| [docs/mcp-tools.md](docs/mcp-tools.md) | MCP Server tool reference (13 tools) |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Development phases and goals |

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Build and test (`make dev && cargo test --workspace && bash tests/integration_lifecycle.sh`)
4. Commit your changes
5. Open a Pull Request

Please read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before contributing to understand the Zig+Rust hybrid architecture.

---

## License

MIT OR Apache-2.0
