# Velos — Roadmap разработки

## Обзор фаз

| Фаза | Название | Scope | Зависимости |
|------|----------|-------|-------------|
| 1 | Skeleton + Build System | Структура проекта, Zig↔Rust FFI, базовый build | — |
| 2 | Core Daemon + Basic CLI | Запуск/остановка процессов, IPC, базовые логи | Phase 1 |
| 3 | Full CLI + Process Management | Все команды PM, авторестарт, watch mode, TOML config | Phase 2 |
| 4 | Smart Logs + Monitoring | Log Engine, TUI monit, метрики CPU/RAM | Phase 3 |
| 5 | AI Features + MCP | MCP Server, --ai флаг, дедупликация, summary, anomaly | Phase 4 |
| 6 | Clustering + Metrics | Кластерный режим, Prometheus, OpenTelemetry | Phase 3 |
| 7 | Polish + Release | Startup scripts, Docker, cross-compile, docs, v0.1.0 | Phase 5, 6 |

---

## Phase 1: Skeleton + Build System

**Цель:** Работающий Zig+Rust гибрид. Zig-библиотека вызывается из Rust.

### Задачи:
- [ ] Инициализировать Git-репозиторий, .gitignore, LICENSE
- [ ] Создать Zig проект (`zig/build.zig`, `zig/src/lib.zig`)
- [ ] Реализовать минимальный C ABI export из Zig (velos_daemon_init, velos_daemon_ping)
- [ ] Создать Rust workspace (`Cargo.toml`, workspace members)
- [ ] Создать crate `velos-ffi` с build.rs для линковки Zig static library
- [ ] Написать safe Rust wrappers над C ABI
- [ ] Создать crate `velos-core` с базовыми типами (ProcessConfig, ProcessStatus)
- [ ] Создать crate `velos-cli` с `clap` — минимальная команда `velos ping`
- [ ] Настроить Makefile (build, test, clean)
- [ ] Верифицировать: `make build` → single binary `velos` → `velos ping` → "pong"

### Результат:
```bash
$ make build
$ ./target/release/velos ping
Velos v0.1.0-dev | Daemon: connected | Uptime: 0s
```

---

## Phase 2: Core Daemon + Basic CLI

**Цель:** Daemon может запускать и останавливать процессы. CLI общается с daemon через IPC.

### Задачи:
- [ ] Zig: Реализовать daemon event loop (epoll/kqueue)
- [ ] Zig: IPC server — Unix socket listener, приём/отправка MessagePack
- [ ] Zig: Process supervisor — fork/exec, SIGCHLD handling, waitpid
- [ ] Zig: Log collector — pipe capture stdout/stderr, запись в файлы
- [ ] Zig: Signal handler — SIGTERM для graceful shutdown daemon'а
- [ ] Rust: `velos-client` — IPC клиент (connect, send, receive)
- [ ] Rust: `velos-core/protocol.rs` — IPC message types
- [ ] CLI: `velos start <script>` — запуск процесса через daemon
- [ ] CLI: `velos stop <name>` — остановка по имени
- [ ] CLI: `velos list` — таблица процессов (name, PID, status)
- [ ] CLI: `velos logs <name>` — чтение логов из файлов
- [ ] CLI: `velos delete <name>` — удаление процесса
- [ ] Daemon auto-start: CLI автоматически запускает daemon если не запущен
- [ ] PID file (~/.velos/velos.pid) для обнаружения daemon'а
- [ ] Базовые тесты: start → list → stop → delete lifecycle

### Результат:
```bash
$ velos start app.js --name api
[OK] Process "api" started (PID: 12345)

$ velos list
┌──────┬────────┬───────┬──────────┐
│ Name │ PID    │ Status│ Uptime   │
├──────┼────────┼───────┼──────────┤
│ api  │ 12345  │ online│ 5s       │
└──────┴────────┴───────┴──────────┘

$ velos logs api
[10:05:03] Server started on port 3000

$ velos stop api
[OK] Process "api" stopped
```

---

## Phase 3: Full CLI + Process Management

**Цель:** Полнофункциональное управление процессами на уровне PM2.

### Задачи:
- [ ] TOML config парсинг (`velos-config` crate)
- [ ] `velos start --config velos.toml` — запуск из конфигурации
- [ ] Авторестарт при падении процесса (configurable)
- [ ] `max_restarts` лимит, exponential backoff
- [ ] `max_memory_restart` — мониторинг памяти, рестарт при превышении
- [ ] `velos restart <name>` — перезапуск
- [ ] `velos reload <name>` — graceful reload (zero-downtime для cluster mode позже)
- [ ] Graceful shutdown: SIGTERM → wait(kill_timeout) → SIGKILL
- [ ] `wait_ready` + process.send('ready') поддержка
- [ ] Watch mode: файловый watcher (inotify/FSEvents), debounce, ignore patterns
- [ ] Переменные окружения: env, env profiles (--env production)
- [ ] Interpreter auto-detection (.js → node, .py → python3, etc.)
- [ ] `velos save` — сохранение текущих процессов
- [ ] `velos resurrect` — восстановление после рестарта
- [ ] Zig: Мониторинг CPU/RAM через syscalls
- [ ] `velos list` — расширенный вывод с CPU/RAM/uptime/restarts
- [ ] `velos info <name>` — детальная информация
- [ ] Cron restart (`cron_restart` option)
- [ ] JSON output: `velos list --json`
- [ ] Ротация логов (size-based, configurable)

### Результат:
```bash
$ velos start --config velos.toml --env production
[OK] Started 3 processes from velos.toml

$ velos list
┌────────┬───────┬────────┬────────┬─────────┬──────────┐
│ Name   │ PID   │ Status │ CPU    │ Memory  │ Restarts │
├────────┼───────┼────────┼────────┼─────────┼──────────┤
│ api    │ 12345 │ online │ 2.1%   │ 45 MB   │ 0        │
│ worker │ 12346 │ online │ 0.3%   │ 12 MB   │ 0        │
│ cron   │ 12347 │ online │ 0.0%   │ 8 MB    │ 0        │
└────────┴───────┴────────┴────────┴─────────┴──────────┘
```

---

## Phase 4: Smart Logs + Monitoring

**Цель:** Smart Log Engine и TUI мониторинг.

### Задачи:
- [ ] `velos-log-engine` crate: базовая инфраструктура
- [ ] Auto-classifier: regex-based level detection
- [ ] Structured JSON Lines формат логов (опциональный)
- [ ] `velos logs <name> --grep <pattern>` — фильтрация
- [ ] `velos logs <name> --level error,warn` — фильтр по уровню
- [ ] `velos logs <name> --since "1h"` — временной фильтр
- [ ] `velos logs <name> --lines 100` — последние N строк
- [ ] Deduplicator: hash-based grouping, `--dedupe` флаг
- [ ] Pattern detector: frequency analysis
- [ ] `velos logs <name> --summary` — компактная сводка
- [ ] `velos monit` — TUI dashboard (ratatui)
- [ ] TUI: таблица процессов с live-обновлением
- [ ] TUI: CPU/RAM графики (sparklines)
- [ ] TUI: live log viewer
- [ ] TUI: keyboard shortcuts (restart, stop, select)
- [ ] WebSocket stream для real-time обновлений

### Результат:
```bash
$ velos logs api --summary
Process: api | Period: last 1h | Health: 92/100
Lines: 3,210 | Errors: 2 | Warnings: 8
Top patterns:
  1. "Request handled in <N>ms" (x2847, stable)
  2. "Cache miss for key <KEY>" (x340, stable)
Last error: "ECONNREFUSED 10.0.0.5:5432" (45min ago)

$ velos monit
# Запускается интерактивный TUI dashboard
```

---

## Phase 5: AI Features + MCP

**Цель:** AI-native интеграция. MCP Server, token-efficient вывод.

### Задачи:
- [ ] `--ai` флаг для всех CLI команд (компактный JSON, сокращённые ключи)
- [ ] `--machine` флаг (полный JSON, но без таблиц/цветов)
- [ ] `velos-mcp` crate: MCP Server (stdio transport)
- [ ] MCP tool: `process_list`
- [ ] MCP tool: `process_start`, `process_stop`, `process_restart`
- [ ] MCP tool: `log_read` (с фильтрацией)
- [ ] MCP tool: `log_search` (regex + time range)
- [ ] MCP tool: `log_summary` (самый важный — экономия токенов)
- [ ] MCP tool: `health_check` (сводка здоровья всех процессов)
- [ ] MCP tool: `metrics_snapshot`
- [ ] MCP tool: `config_get`, `config_set`
- [ ] `velos mcp-server` команда для запуска MCP сервера
- [ ] Anomaly detector: статистический (sliding window, sigma)
- [ ] Health score: эвристика (0-100) на основе метрик и логов
- [ ] Документация MCP tools (JSON Schema для каждого tool)

### Результат:
```bash
$ velos list --ai
[{"n":"api","s":"on","c":2.1,"m":"45M","u":"2d"}]

$ velos mcp-server
# MCP сервер запущен, AI-агенты могут подключаться

# В claude settings:
# "mcpServers": { "velos": { "command": "velos", "args": ["mcp-server"] } }
```

---

## Phase 6: Clustering + Metrics

**Цель:** Multi-instance management и production observability.

### Задачи:
- [ ] Cluster mode: `velos start app.js -i 4` (запуск N инстансов)
- [ ] Load balancing между инстансами (round-robin)
- [ ] Rolling restart для zero-downtime
- [ ] `NODE_APP_INSTANCE` / `VELOS_INSTANCE_ID` env variable
- [ ] `velos scale <name> +2` / `velos scale <name> 8` — масштабирование
- [ ] `velos-metrics` crate
- [ ] Prometheus exporter: `velos metrics --port 9615`
- [ ] Все метрики per-process + daemon-level
- [ ] OpenTelemetry integration (traces, spans, resource attributes)
- [ ] OTLP exporter (gRPC/HTTP)
- [ ] `velos-api` crate: REST API (axum)
- [ ] REST: GET /api/processes, GET /api/logs/:name, POST /api/processes
- [ ] WebSocket: real-time process updates
- [ ] API authentication (optional, token-based)

### Результат:
```bash
$ velos start app.js -i 4 --name api
[OK] Started 4 instances of "api"

$ velos scale api 8
[OK] Scaled "api" from 4 to 8 instances

$ curl localhost:9615/metrics
# Prometheus метрики
```

---

## Phase 7: Polish + Release (v0.1.0)

**Цель:** Production-ready release.

### Задачи:
- [ ] `velos startup` — генерация systemd/launchd service files
- [ ] Systemd watchdog integration (sd_notify)
- [ ] Docker image (multi-stage build, Alpine-based)
- [ ] `velos-runtime` (аналог pm2-runtime для Docker)
- [ ] Cross-compilation для всех 5 targets
- [ ] GitHub Actions CI/CD pipeline
- [ ] Homebrew formula
- [ ] AUR package
- [ ] cargo install support
- [ ] Shell completions (bash, zsh, fish)
- [ ] Man pages
- [ ] README.md с примерами и бенчмарками
- [ ] Performance benchmarks (vs PM2, vs PMDaemon)
  - Startup time
  - Daemon memory usage
  - IPC latency
  - Process spawn time
- [ ] Security audit (socket permissions, signal handling)
- [ ] Error messages polishing
- [ ] `velos --version`, `velos --help` polishing
- [ ] CHANGELOG.md
- [ ] Тег v0.1.0 + GitHub Release

### Результат:
```bash
$ velos --version
Velos 0.1.0 (zig-core 0.1.0, rust-shell 0.1.0)
Platform: aarch64-apple-darwin
Daemon RAM: 1.2 MB

$ brew install velos
$ cargo install velos
```

---

## Сводная диаграмма зависимостей

```
Phase 1 (Skeleton)
    │
    ▼
Phase 2 (Core Daemon)
    │
    ▼
Phase 3 (Full CLI) ─────────────┐
    │                            │
    ├──────────┐                 │
    ▼          ▼                 ▼
Phase 4     Phase 6          Phase 6
(Logs+TUI)  (Cluster)       (Metrics)
    │          │                 │
    ▼          └────────┬────────┘
Phase 5                 │
(AI+MCP)                │
    │                   │
    └───────────────────┘
              │
              ▼
         Phase 7
       (Polish+Release)
              │
              ▼
          v0.1.0
```
