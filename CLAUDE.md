# Velos — Project Guide

## What is this

Velos is a high-performance AI-friendly process manager (next-gen PM2 alternative). Zig core + Rust shell. Cross-platform (macOS, Linux). First process manager with a native MCP server and algorithmic smart log analysis at zero LLM cost.

## Architecture (Zig + Rust hybrid)

**Zig core (`zig/src/`)**: daemon, fork/exec, IPC server (Unix socket), CPU/RAM monitoring via syscalls, log collector with error pattern detection, ring buffer. Exports C ABI as static library `libvelos_core.a`.

**Rust shell (`crates/`)**: CLI (clap), IPC client, REST API (axum), MCP Server (stdio + Streamable HTTP), Smart Log Engine (dedup, anomaly detection, pattern detection, summary), Prometheus/OpenTelemetry exporter, TOML config parser, AI crash analysis agent.

**Bridge**: Zig compiles to static lib, Rust links via FFI (C ABI). Build: `zig build` -> `cargo build` (velos-ffi/build.rs links `.a`).

### Crates

| Crate | Role |
|-------|------|
| `velos-ffi` | FFI bindings to Zig (extern "C", safe wrappers) |
| `velos-core` | Shared types: ProcessConfig, ProcessState, IPC protocol, errors |
| `velos-client` | IPC client (Unix socket -> daemon) |
| `velos-config` | TOML parsing and validation |
| `velos-log-engine` | Smart logs: dedup, patterns, anomaly, classifier, summary |
| `velos-metrics` | Prometheus exporter, OpenTelemetry |
| `velos-mcp` | MCP Server (stdio + HTTP, JSON-RPC, 13 tools) |
| `velos-api` | REST API + WebSocket (axum) |
| `velos-ai` | AI crash analysis, agent with 9 tools, multi-provider (Anthropic/OpenAI) |
| `velos-cli` | CLI binary (clap, ratatui TUI) |

### IPC Protocol

Binary: 7-byte header (magic `0xVE10` + version + length LE u32) + MessagePack payload. Unix socket `~/.velos/velos.sock`. Commands: PROCESS_START (0x01), STOP (0x02), RESTART (0x03), LIST (0x05), LOG_READ (0x10), LOG_STREAM (0x11), METRICS_GET (0x20), etc.

## Key documentation

| File | What | When to read |
|------|------|--------------|
| `docs/ARCHITECTURE.md` | Full architecture (1200+ lines): components, IPC protocol, data models, Smart Log Engine algorithms, MCP tools, metrics, build system | Before implementing any module |
| `docs/mcp-tools.md` | MCP Server tool reference (13 tools) | Before working on MCP tools |

## Build commands

```bash
make build          # Zig + Rust release build
make dev            # Zig debug + Rust debug (fast iteration)
make test           # All tests (Zig + Rust)
make clean          # Clean everything
```

Pipeline: `zig build` -> `libvelos_core.a` -> `cargo build` -> `target/release/velos`

## Project conventions

- **Language**: Zig for low-level daemon core, Rust for everything else
- **Config format**: TOML (`velos.toml`)
- **IPC**: Binary protocol over Unix socket (MessagePack payload)
- **Error handling**: Zig returns int error codes via C ABI; Rust uses thiserror
- **Async**: Rust uses tokio; Zig uses manual epoll/kqueue event loop
- **Testing**: Zig `zig build test`; Rust `cargo test`; integration tests in `tests/`
- **AI output**: `--ai` flag = compact JSON with abbreviated keys (n=name, s=status, c=cpu, m=memory)
- **AI principle**: All "smart" features (dedup, anomaly, patterns) are algorithmic — zero LLM cost

## Runtime directory

```
~/.velos/
├── velos.sock          # IPC socket
├── velos.pid           # Daemon PID
├── config.toml         # Global config (velos config set/get)
├── state.bin           # Saved process state (velos save / auto-save)
├── crashes/            # AI crash records and agent logs
└── logs/               # Process logs ({name}-out.log, {name}-err.log)
```

## Integration tests

New features must have integration tests in `tests/integration_lifecycle.sh`.

Checklist:
1. Add tests BEFORE the shutdown test (always keep shutdown as the last test)
2. Run `make dev && bash tests/integration_lifecycle.sh` to verify
3. Update the test count in the file header comment

## Common patterns

When implementing a new CLI command:
1. Add IPC message type in `velos-core/src/protocol.rs`
2. Handle command in Zig daemon (`zig/src/ipc/server.zig`)
3. Add client method in `velos-client/src/commands.rs`
4. Add CLI subcommand in `velos-cli/src/commands/{cmd}.rs`
5. Support `--json` and `--ai` output modes

When implementing a new MCP tool:
1. Add tool definition in `velos-mcp/src/tools.rs`
2. Add input/output schema in `velos-mcp/src/schema.rs`
3. Use `velos-client` for daemon communication (don't duplicate IPC logic)
4. Return minimal data (token-efficient)
