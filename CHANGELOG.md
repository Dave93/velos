# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.14] - 2026-03-12

### Added
- Ready-to-run example configs for Node.js, Python FastAPI, Rust binary, and multi-process stacks (`examples/`)
- Each example includes working application code, `velos.toml`, and README with quick start instructions

### Fixed
- Config-based start (`--config`) now resolves `cwd` relative to config file directory instead of daemon's working directory
- Resolve clippy warnings for Rust 1.94

## [0.1.13] - 2026-03-07

### Added
- CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md for open-source community
- GitHub issue templates (bug report, feature request) and PR template
- `.editorconfig` for consistent formatting across editors

### Changed
- README: rewrite "Why Velos?" section with narrative and accurate benchmarks (~3 MB measured)
- README: add Runtime Error Detection, auto-daemon, Telegram crash alerts documentation
- README: fix integration test count (35 -> 49), update runtime directory structure
- CLAUDE.md: translate to English, remove internal workflow instructions
- Expand .gitignore with IDE, OS, and environment file patterns

### Removed
- Internal development docs: task checklists, design plans, concept, roadmap (not needed for public repo)

## [0.1.12] - 2026-03-07

### Added
- Sentry-like runtime error detection — monitor stderr for error patterns (Traceback, TypeError, panic, FATAL, etc.) and send Telegram notifications without requiring process crash
- Suppress crash/error notifications after AI fix restart (marker file mechanism)

### Fixed
- Prevent notification spam on autorestart loops — 60s cooldown per process for crash notifications
- Skip error notification if process already crashed (avoid duplicate alerts)
- Non-blocking fix execution in Telegram poller (thread-based, no longer blocks callback processing)

## [0.1.11] - 2026-03-06

### Added
- Auto-start daemon on any CLI command — no need to run `velos daemon` or `velos startup` manually

### Fixed
- Fix `velos startup` / `velos unstartup` on modern macOS — use `launchctl bootstrap/bootout` instead of deprecated `load/unload`

## [0.1.10] - 2026-03-06

### Added
- AI crash analysis and auto-fix agent (`velos-ai` crate) — autonomous coding agent with 9 tools (read/edit/create/delete files, grep, glob, list_dir, run_command, git_diff)
- Support for Anthropic (Claude) and OpenAI-compatible AI providers
- CLI commands: `velos ai fix`, `velos ai analyze`, `velos ai list`, `velos ai ignore`
- Telegram inline buttons (Fix / Ignore) for crash notifications with callback polling
- Auto-restart process after successful AI fix
- Per-crash agent logs at `~/.velos/crashes/<id>.log`
- i18n support (EN/RU) for all AI-related messages
- Path sandboxing for AI agent tools (restricted to project directory)

## [0.1.9] - 2026-03-05

### Added
- Redesign `velos monit` TUI dashboard — Catppuccin Mocha color scheme, dual CPU/Memory sparkline graphs, inline CPU gauge bars, auto-streaming logs with colored stderr/stdout tags, filter mode (`/`), sort by column (`Tab`), detail panel (`Enter`), signal picker (`k`), help popup (`?`), toast crash notifications, vim navigation (`j`/`k`)
- Telegram crash notifications — daemon fork+exec sends alerts to Telegram on process crash with last log lines
- `velos config set/get` command for global daemon settings (`~/.velos/config.toml`)
- `signal()` method in VelosClient for sending arbitrary signals to processes

### Fixed
- Use-after-free in argv when applying env vars before exec (interpreter string freed by defer inside if block before fork)

## [0.1.8] - 2026-03-05

### Added
- Pass user shell environment to child processes via IPC — fixes processes failing under launchd daemon due to minimal PATH
- Exec failure error reporting: write error details to stderr (visible in `velos logs`) when `exec` fails
- Persist env vars in `state.bin` for `velos save`/`velos resurrect`

## [0.1.7] - 2026-03-05

### Added
- CPU% monitoring in `velos list` — delta-based calculation every ~2s
- macOS: CPU time via `proc_pid_rusage` with Mach timebase conversion
- Linux: CPU time via `/proc/[pid]/stat` (utime + stime)
- Real CPU data in Prometheus metrics (`velos_process_cpu_percent`)

## [0.1.6] - 2026-03-05

### Fixed
- Preserve error logs across process restarts (ring buffer no longer cleared on autorestart)
- Show crash warning after `velos start` if process fails immediately

### Changed
- Upgrade `velos list` to modern table with rounded borders (`tabled` library)
- Color-coded statuses: green (running), red (errored), yellow (stopped)
- Color-coded mode: cyan (fork), blue (cluster)
- Bold table headers, compact memory/uptime formatting
- Added `mode` column (fork/cluster)

## [0.1.5] - 2026-03-05

### Added
- `--cwd` flag for `velos start` to specify working directory
- `--interpreter` flag for `velos start` to override interpreter detection
- `/release` slash command for automated version releases in Claude Code

### Changed
- Expanded `--ai` output mode documentation with examples and key reference table
- Added MCP setup instructions for OpenAI Codex and Gemini CLI

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
