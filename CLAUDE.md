# Velos — Project Guide

## What is this

Velos — высокопроизводительный AI-friendly процесс-менеджер (аналог PM2). Zig-ядро + Rust-оболочка. Кросс-платформенный. Первый process manager с нативным MCP-сервером и алгоритмической AI-оптимизацией логов без расходов на LLM.

## Architecture (Zig + Rust hybrid)

**Zig-ядро (`zig/src/`)**: daemon, fork/exec, IPC server (Unix socket), CPU/RAM мониторинг через syscalls, log collector, ring buffer, custom allocator. Экспортирует C ABI → static library `libvelos_core.a`.

**Rust-оболочка (`crates/`)**: CLI (clap), IPC client, REST API (axum), MCP Server (stdio), Smart Log Engine (dedup, anomaly detection, pattern detection, summary), Prometheus/OpenTelemetry exporter, TOML config parser.

**Связь**: Zig → static lib → Rust FFI через C ABI. Build: `zig build` → `cargo build` (velos-ffi/build.rs линкует `.a`).

### Crates

| Crate | Role |
|-------|------|
| `velos-ffi` | FFI bindings к Zig (extern "C", safe wrappers) |
| `velos-core` | Shared types: ProcessConfig, ProcessState, IPC protocol, errors |
| `velos-client` | IPC клиент (Unix socket → daemon) |
| `velos-config` | TOML парсинг и валидация |
| `velos-log-engine` | Smart logs: dedup, patterns, anomaly, classifier, summary |
| `velos-metrics` | Prometheus exporter, OpenTelemetry |
| `velos-mcp` | MCP Server (stdio, JSON-RPC) |
| `velos-api` | REST API + WebSocket (axum) |
| `velos-cli` | CLI binary (clap, ratatui TUI) |

### IPC Protocol

Binary: 7-byte header (magic `0xVE10` + version + length LE u32) + MessagePack payload. Unix socket `~/.velos/velos.sock`. Команды: PROCESS_START (0x01), STOP (0x02), RESTART (0x03), LIST (0x05), LOG_READ (0x10), LOG_STREAM (0x11), METRICS_GET (0x20), etc.

## Key documentation

| File | What | When to read |
|------|------|--------------|
| `docs/CONCEPT.md` | Vision, features, competitor comparison | Before any design decision |
| `docs/ARCHITECTURE.md` | Full architecture (1246 lines): components, IPC protocol, data models, Smart Log Engine algorithms, MCP tools, metrics, build system | Before implementing any module |
| `docs/ROADMAP.md` | Phase descriptions, goals, expected results | To understand phase context |
| `docs/TASKS.md` | Progress tracker (overview table) | To see overall progress |
| `docs/tasks/phase-N-*.md` | Detailed task checklists per phase | Before starting work on a phase |

## Task system

Tasks live in `docs/tasks/phase-{0..7}-*.md` as markdown checklists. `docs/TASKS.md` is the overview.

### Working with tasks

1. **Before starting work**: read `docs/TASKS.md` to find the current active phase (first non-Done phase)
2. **Before a phase**: read `docs/tasks/phase-N-*.md` for the full task list and notes
3. **Before a task**: read `docs/ARCHITECTURE.md` relevant section for design details
4. **After completing a task**: mark `[ ]` → `[x]` in the phase file
5. **After completing a phase**: update status in `docs/TASKS.md` (progress + "Done")
6. **If blocked**: add `[BLOCKED] reason` next to the task
7. **If skipping**: add `[SKIP] reason` next to the task

### Phase dependencies

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
                          └──→ Phase 6 ──────┘
                                              └→ Phase 7 (v0.1.0)
```

Phases 4 and 6 can run in parallel after Phase 3.

### Counting progress

```bash
# Count tasks per phase:
for f in docs/tasks/phase-*.md; do
  total=$(grep -c '^\- \[' "$f")
  done=$(grep -c '^\- \[x\]' "$f" || true)
  printf "%-30s %d/%d\n" "$(basename "$f")" "$done" "$total"
done
```

## Build commands

```bash
make build          # Zig + Rust release build
make dev            # Zig debug + Rust debug (fast iteration)
make test           # All tests (Zig + Rust)
make clean          # Clean everything
```

Pipeline: `zig build` → `libvelos_core.a` → `cargo build` → `target/release/velos`

## Project conventions

- **Language**: Zig for low-level daemon core, Rust for everything else
- **Config format**: TOML (`velos.toml`)
- **IPC**: Binary protocol over Unix socket (MessagePack payload)
- **Logs**: Structured JSON Lines (optional, with plain text fallback)
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
├── config.toml         # Global config
├── state.json          # Saved process list (velos save)
└── logs/               # Process logs ({name}-out.log, {name}-err.log)
```

## Workflow — Teammates

All implementation work MUST use teammates (parallel agents) whenever possible. When starting a phase or a group of tasks:

1. **Create a team** (TeamCreate) for the current phase/work scope
2. **Break work into parallel tasks** — identify independent tasks that can be done simultaneously (e.g., Zig and Rust parts of the same phase, independent modules, tests)
3. **Spawn teammates** for each independent work stream (use `general-purpose` subagent_type for implementation tasks)
4. **Coordinate via task list** — use TaskCreate/TaskUpdate for tracking, SendMessage for communication
5. **Only work sequentially** when there is a hard dependency (e.g., Zig static lib must exist before Rust FFI can link against it)

Examples of parallelizable work:
- Zig event loop (2.1) + Rust IPC types in velos-core (2.6) — no dependency until linking
- Multiple independent CLI commands
- Unit tests for different modules
- Documentation + implementation

## Integration tests — ОБЯЗАТЕЛЬНЫ

After completing each phase, you MUST add integration tests to `tests/integration_lifecycle.sh` covering all new user-facing features. This is NOT optional — phase is not complete until integration tests pass.

Checklist:
1. **Identify testable features** — every new CLI flag, IPC command, or behavior change needs a test
2. **Add tests BEFORE the shutdown test** (always keep shutdown as the last test)
3. **Renumber the shutdown test** accordingly
4. **Run `make dev && bash tests/integration_lifecycle.sh`** to verify all tests pass
5. **Update the test count** in the file header comment

If a feature is untestable in non-interactive mode (e.g. TUI), document it as "[MANUAL]" in the phase task file.

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
