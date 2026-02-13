# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-13

### Added
- Core daemon with fork/exec process management (Zig kernel, ~2MB idle RAM)
- CLI with 20+ commands: start, stop, restart, reload, list, info, logs, delete, save, resurrect, flush, scale, metrics, api, monit, completions, startup, ping
- Binary IPC protocol over Unix socket (7-byte header + MessagePack payload)
- Autorestart with crash loop detection (max_restarts, min_uptime, exp_backoff)
- Cluster mode with multi-instance support (`-i N` or `-i max`)
- Smart Log Engine (zero LLM cost):
  - Auto-classifier (regex + JSON-aware level detection)
  - Deduplication (normalize IPs/UUIDs/numbers, sliding window grouping)
  - Pattern detection (frequency analysis, trend: rising/stable/declining)
  - Anomaly detection (sliding window, mean + stddev, 2σ/3σ thresholds)
  - Summary with health score (0-100)
- MCP server with 13 tools for AI agent integration (stdio, JSON-RPC 2.0)
- Prometheus metrics endpoint (`/metrics`) with OpenTelemetry support
- REST API with WebSocket real-time updates (axum)
- TOML configuration with env profiles (`--env production`)
- Watch mode — file change detection via kqueue/inotify → auto-restart
- Cron-based periodic restart (`--cron-restart "0 3 * * *"`)
- Process ready signal (`--wait-ready` + `VELOS_IPC_FD`)
- Graceful shutdown via IPC messages (`--shutdown-with-message`)
- Max memory restart (`--max-memory 150M`)
- State persistence — save/resurrect with auto-save on start/stop
- TUI monitoring dashboard (`velos monit`) with ratatui
- Shell completions for bash, zsh, fish, elvish, powershell
- Startup script generation for systemd/launchd (`velos startup`)
- `--json` and `--ai` output modes for machine/LLM consumption
- Interpreter auto-detection (node, python3, ruby, deno, direct exec)
- Log rotation (size-based, configurable retain count)

### Architecture
- Zig core for low-level daemon (fork/exec, kqueue/epoll, syscall monitoring)
- Rust shell for CLI, networking, smart log algorithms, MCP server
- C ABI bridge: Zig → `libvelos_core.a` → Rust FFI
- Binary IPC protocol: `0xVE10` magic + version + MessagePack
- Token-efficient MCP responses (89 tokens vs 2,847 for PM2 equivalent)
