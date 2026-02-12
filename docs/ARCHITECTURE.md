# Velos — Архитектура

## 1. Высокоуровневая архитектура компонентов

```
┌─────────────────────────────────────────────────────────────────┐
│                        Пользователь / AI-агент                  │
└──────┬──────────────┬───────────────┬───────────────┬───────────┘
       │              │               │               │
       ▼              ▼               ▼               ▼
┌──────────┐  ┌──────────────┐ ┌────────────┐ ┌──────────────┐
│ velos    │  │ REST API     │ │ MCP Server │ │ Prometheus   │
│ CLI      │  │ + WebSocket  │ │ (stdio)    │ │ /metrics     │
│ (Rust)   │  │ (Rust/axum)  │ │ (Rust)     │ │ (Rust)       │
└────┬─────┘  └──────┬───────┘ └─────┬──────┘ └──────┬───────┘
     │               │               │               │
     └───────────────┴───────┬───────┴───────────────┘
                             │
                    ┌────────▼────────┐
                    │  velos-client   │
                    │  (Rust)        │
                    │  IPC клиент    │
                    └────────┬───────┘
                             │ Unix Socket / Named Pipe
                             │ (Binary protocol)
                    ┌────────▼────────┐
                    │  velos-daemon   │
                    │  (Zig core)    │
                    │                │
                    │ ┌────────────┐ │
                    │ │ Process    │ │
                    │ │ Supervisor │ │
                    │ ├────────────┤ │
                    │ │ IPC Server │ │
                    │ ├────────────┤ │
                    │ │ Monitor    │ │
                    │ │ (CPU/RAM)  │ │
                    │ ├────────────┤ │
                    │ │ Log        │ │
                    │ │ Collector  │ │
                    │ ├────────────┤ │
                    │ │ State      │ │
                    │ │ Persistence│ │
                    │ └────────────┘ │
                    └───────┬────────┘
                            │ fork/exec + signals
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
         ┌─────────┐ ┌─────────┐ ┌─────────┐
         │ Child   │ │ Child   │ │ Child   │
         │ Process │ │ Process │ │ Process │
         │ (app.js)│ │ (api.py)│ │ (server)│
         └─────────┘ └─────────┘ └─────────┘
```

## 1.1 Компоненты и зоны ответственности

### Zig-ядро (libvelos-core)

Статическая библиотека, экспортирующая C ABI. Минимальный footprint, zero allocations где возможно.

| Модуль | Ответственность |
|---|---|
| **process_supervisor** | fork/exec дочерних процессов, reaping (waitpid), отслеживание PID, отправка сигналов (SIGTERM/SIGKILL/SIGUSR1), авторестарт |
| **ipc_server** | Unix domain socket listener, приём и отправка бинарных сообщений, мультиплексирование клиентов (epoll/kqueue) |
| **monitor** | Сбор метрик CPU/RAM через прямые syscalls (getrusage, /proc/[pid]/stat на Linux, task_info на macOS), периодический polling |
| **log_collector** | Перехват stdout/stderr дочерних процессов через pipe, запись в файлы с ротацией, ring buffer для последних N строк в памяти |
| **state_manager** | Сохранение/загрузка состояния процессов на диск (JSON), восстановление после рестарта daemon |
| **allocator** | Custom arena/pool allocator для предсказуемого потребления памяти, предотвращение фрагментации |
| **signal_handler** | Обработка сигналов для daemon (SIGCHLD, SIGTERM, SIGHUP для graceful reload конфигурации) |

### Rust-оболочка

| Crate | Ответственность |
|---|---|
| **velos-cli** | Парсинг команд (clap), красивый вывод (tabled/comfy-table), --ai/--json режимы, TUI монитор (ratatui) |
| **velos-client** | IPC клиент для общения с daemon через Unix socket, сериализация/десериализация сообщений |
| **velos-api** | REST API (axum) + WebSocket для real-time обновлений, CORS, auth (опционально) |
| **velos-mcp** | MCP Server (stdio transport), определение tools, обработка запросов от AI-агентов |
| **velos-log-engine** | Smart Log Engine: дедупликация, pattern detection, anomaly detection, auto-classification, summary generation |
| **velos-metrics** | Prometheus exporter (/metrics endpoint), OpenTelemetry SDK integration (traces, spans) |
| **velos-config** | Парсинг TOML конфигурации (serde + toml crate), валидация, merge с CLI аргументами |
| **velos-ffi** | Rust bindings к Zig static library через C ABI (extern "C"), safe wrapper types |

## 1.2 C ABI интерфейс (Zig <-> Rust)

```c
// === Exported from Zig (libvelos_core.h) ===

// Lifecycle
int velos_daemon_init(const char* socket_path, const char* state_dir);
int velos_daemon_run(void);  // main event loop (blocking)
int velos_daemon_shutdown(void);

// Process management
int velos_process_start(const VelosProcessConfig* config);  // returns process_id
int velos_process_stop(uint32_t process_id, int signal, uint32_t timeout_ms);
int velos_process_restart(uint32_t process_id);
int velos_process_delete(uint32_t process_id);

// Monitoring
int velos_process_get_metrics(uint32_t process_id, VelosMetrics* out);
int velos_process_list(VelosProcessInfo** out, uint32_t* count);
void velos_process_list_free(VelosProcessInfo* list, uint32_t count);

// Logs
int velos_log_read(uint32_t process_id, uint32_t lines, VelosLogEntry** out, uint32_t* count);
void velos_log_free(VelosLogEntry* entries, uint32_t count);

// State
int velos_state_save(const char* path);
int velos_state_load(const char* path);

// === Structures ===

typedef struct {
    const char* name;
    const char* script;
    const char* cwd;
    const char* interpreter;       // NULL = auto-detect
    const char** env_keys;
    const char** env_values;
    uint32_t env_count;
    uint32_t instances;            // 0 = 1 (fork mode), >1 = cluster mode
    uint64_t max_memory_bytes;     // 0 = unlimited
    uint32_t kill_timeout_ms;      // default: 5000
    int32_t  max_restarts;         // -1 = unlimited
    bool     autorestart;
} VelosProcessConfig;

typedef struct {
    uint32_t id;
    const char* name;
    uint32_t pid;
    uint8_t  status;               // 0=stopped, 1=running, 2=errored, 3=starting
    uint64_t cpu_percent_x100;     // CPU% * 100 (e.g. 1550 = 15.50%)
    uint64_t memory_bytes;
    uint64_t uptime_ms;
    uint32_t restart_count;
} VelosProcessInfo;

typedef struct {
    uint64_t cpu_percent_x100;
    uint64_t memory_bytes;
    uint64_t memory_peak_bytes;
    uint64_t uptime_ms;
    uint32_t restart_count;
    uint64_t timestamp_ms;
} VelosMetrics;

typedef struct {
    uint64_t timestamp_ms;
    uint8_t  level;                // 0=debug, 1=info, 2=warn, 3=error, 4=fatal
    uint8_t  stream;               // 0=stdout, 1=stderr
    const char* message;
    uint32_t message_len;
} VelosLogEntry;
```

## 1.3 Потоки данных

### Запуск процесса
```
CLI: velos start app.js --name api -i 4
  │
  ▼
velos-cli: парсит аргументы, формирует VelosProcessConfig
  │
  ▼
velos-client: сериализует в IPC message, отправляет в Unix socket
  │
  ▼
velos-daemon (Zig): ipc_server получает команду
  │
  ▼
process_supervisor: fork() N раз, exec() в каждом child
  │
  ▼
log_collector: создаёт pipe для каждого child, начинает захват stdout/stderr
  │
  ▼
monitor: регистрирует PID, начинает периодический сбор метрик
  │
  ▼
state_manager: сохраняет конфигурацию на диск
  │
  ▼
ipc_server: отправляет ответ с process_id и статусом
  │
  ▼
velos-cli: выводит результат пользователю
```

### AI-агент через MCP
```
AI Agent (Claude/Cursor): вызывает MCP tool "log_search"
  │
  ▼
velos-mcp: получает запрос через stdio, парсит параметры
  │
  ▼
velos-client: отправляет IPC запрос на чтение логов с фильтрами
  │
  ▼
velos-daemon (Zig): log_collector читает из ring buffer / файлов
  │
  ▼
velos-log-engine (Rust): дедупликация + pattern detection + summary
  │
  ▼
velos-mcp: формирует компактный ответ (минимум токенов)
  │
  ▼
AI Agent: получает только релевантные данные
```

## 1.4 Гибридный Daemon

### Режим 1: Standalone (по умолчанию)
```
velos start app.js
  └─> запускает velos-daemon если не запущен
      └─> daemon слушает ~/.velos/velos.sock
          └─> fork/exec child processes
```

### Режим 2: Systemd/Launchd integration
```
velos startup --systemd
  └─> генерирует velos-daemon.service
      └─> systemd управляет lifecycle daemon'а
          └─> daemon делегирует watchdog systemd
              └─> sd_notify(READY=1), sd_notify(WATCHDOG=1)
```

## 1.5 Кросс-платформенная абстракция (Zig)

```
┌─────────────────────────────────────┐
│        velos-core (Zig)             │
│                                     │
│  ┌────────────────────────────┐     │
│  │    Platform Abstraction    │     │
│  │         Layer (PAL)        │     │
│  ├──────┬─────────┬───────────┤     │
│  │Linux │  macOS  │  Windows  │     │
│  │------│---------│-----------│     │
│  │epoll │ kqueue  │ IOCP     │     │
│  │fork  │ fork    │ CreatePr │     │
│  │/proc │task_info│ NtQuery  │     │
│  │unix  │ unix    │ named    │     │
│  │socket│ socket  │ pipe     │     │
│  │sigfd │ kqueue  │ events   │     │
│  └──────┴─────────┴───────────┘     │
└─────────────────────────────────────┘
```

---

## 2. Структура директорий и модулей проекта

```
velos/
├── Cargo.toml                    # Rust workspace root
├── build.rs                      # Workspace-level: запуск Zig build, линковка libvelos_core
├── Makefile                      # Верхнеуровневые таргеты: build, test, release, clean
│
├── zig/                          # === Zig-ядро ===
│   ├── build.zig                 # Zig build script → выход: libvelos_core.a
│   ├── build.zig.zon             # Zig package manifest
│   └── src/
│       ├── main.zig              # Entry point для standalone daemon binary
│       ├── lib.zig               # Export C ABI functions (корень библиотеки)
│       ├── process/
│       │   ├── supervisor.zig    # fork/exec, PID tracking, авторестарт
│       │   ├── signals.zig       # SIGCHLD, SIGTERM, SIGHUP handling
│       │   └── cluster.zig       # Multi-instance management, load balancing
│       ├── ipc/
│       │   ├── server.zig        # Unix socket listener, client multiplexing
│       │   ├── protocol.zig      # Message format, serialize/deserialize
│       │   └── platform.zig      # Platform-specific transport (socket vs named pipe)
│       ├── monitor/
│       │   ├── collector.zig     # CPU/RAM metrics collection
│       │   ├── platform_linux.zig
│       │   ├── platform_macos.zig
│       │   └── platform_windows.zig
│       ├── log/
│       │   ├── collector.zig     # Pipe capture stdout/stderr
│       │   ├── writer.zig        # File writer with rotation
│       │   └── ring_buffer.zig   # In-memory ring buffer для последних N строк
│       ├── state/
│       │   ├── persistence.zig   # Save/load state to disk (JSON)
│       │   └── recovery.zig      # Crash recovery, orphan process detection
│       ├── memory/
│       │   ├── arena.zig         # Arena allocator
│       │   └── pool.zig          # Pool allocator для фиксированных структур
│       └── platform/
│           ├── pal.zig           # Platform Abstraction Layer interface
│           ├── linux.zig         # Linux-specific: epoll, signalfd, /proc
│           ├── macos.zig         # macOS-specific: kqueue, task_info
│           └── windows.zig       # Windows-specific: IOCP, CreateProcess, named pipes
│
├── crates/                       # === Rust crates ===
│   ├── velos-ffi/                # FFI bindings к Zig
│   │   ├── Cargo.toml
│   │   ├── build.rs              # Линковка libvelos_core.a, генерация bindings
│   │   └── src/
│   │       ├── lib.rs            # extern "C" declarations, safe wrappers
│   │       └── types.rs          # Rust-эквиваленты C структур (repr(C))
│   │
│   ├── velos-core/               # Shared types и логика
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── process.rs        # ProcessConfig, ProcessState, ProcessStatus enums
│   │       ├── config.rs         # TOML config parsing, VelosConfig struct
│   │       ├── error.rs          # Unified error types (thiserror)
│   │       └── protocol.rs       # IPC message types (shared between client/daemon)
│   │
│   ├── velos-client/             # IPC клиент
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs     # Unix socket connection management
│   │       └── commands.rs       # High-level command API (start, stop, logs, etc.)
│   │
│   ├── velos-log-engine/         # Smart Log Engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dedup.rs          # Log deduplication (hash-based grouping)
│   │       ├── pattern.rs        # Pattern detection (frequency analysis)
│   │       ├── anomaly.rs        # Anomaly detection (statistical)
│   │       ├── classifier.rs     # Auto-classification (regex rules)
│   │       ├── summary.rs        # Compact digest generator
│   │       └── search.rs         # Full-text + regex search engine
│   │
│   ├── velos-metrics/            # Observability
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── prometheus.rs     # Prometheus /metrics endpoint
│   │       └── otel.rs           # OpenTelemetry traces/spans
│   │
│   ├── velos-mcp/                # MCP Server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs         # MCP stdio transport, JSON-RPC handler
│   │       ├── tools.rs          # Tool definitions (process_list, log_search, etc.)
│   │       └── schema.rs         # Input/output schemas for each tool
│   │
│   ├── velos-api/                # REST API + WebSocket
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── routes.rs         # HTTP routes (/api/processes, /api/logs, etc.)
│   │       ├── websocket.rs      # WebSocket real-time updates
│   │       └── middleware.rs     # CORS, auth, rate limiting
│   │
│   └── velos-cli/                # CLI application (main binary)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # Entry point, subcommand routing
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── start.rs      # velos start
│           │   ├── stop.rs       # velos stop
│           │   ├── restart.rs    # velos restart
│           │   ├── delete.rs     # velos delete
│           │   ├── list.rs       # velos list
│           │   ├── logs.rs       # velos logs (with --grep, --dedupe, --summary)
│           │   ├── monit.rs      # velos monit (TUI dashboard)
│           │   ├── save.rs       # velos save
│           │   ├── startup.rs    # velos startup (systemd/launchd generation)
│           │   ├── reload.rs     # velos reload
│           │   ├── metrics.rs    # velos metrics (Prometheus server)
│           │   └── mcp.rs        # velos mcp-server (MCP запуск)
│           ├── output/
│           │   ├── mod.rs
│           │   ├── table.rs      # Human-readable table output
│           │   ├── json.rs       # --json full output
│           │   └── ai.rs         # --ai compact output (token-efficient)
│           └── tui/
│               ├── mod.rs
│               ├── dashboard.rs  # Main TUI layout (ratatui)
│               ├── process_view.rs
│               └── log_view.rs
│
├── include/                      # Generated C headers
│   └── velos_core.h              # Auto-generated from Zig exports
│
├── config/                       # Example configs
│   ├── velos.example.toml        # Пример конфигурации
│   └── ecosystem.example.toml    # Пример multi-app config
│
├── tests/                        # Integration tests
│   ├── integration/
│   │   ├── test_process_lifecycle.rs
│   │   ├── test_cluster_mode.rs
│   │   ├── test_log_engine.rs
│   │   ├── test_mcp_server.rs
│   │   └── test_ai_output.rs
│   └── fixtures/
│       ├── sample_app.js
│       ├── sample_app.py
│       └── crash_app.js
│
├── .github/
│   └── workflows/
│       ├── ci.yml                # Build + test on Linux/macOS/Windows
│       └── release.yml           # Cross-compile + publish binaries
│
└── docs/
    ├── CONCEPT.md                # Видение, позиционирование, фичи
    ├── ARCHITECTURE.md           # Архитектура (этот файл)
    ├── ROADMAP.md                # Описание фаз, цели, результаты
    ├── TASKS.md                  # Трекер задач (общий прогресс)
    ├── mcp-tools.md              # MCP tools reference
    └── tasks/                    # Детальные чеклисты задач по фазам
        └── phase-{0..7}-*.md
```

### Cargo workspace (Cargo.toml)

```toml
[workspace]
resolver = "2"
members = [
    "crates/velos-ffi",
    "crates/velos-core",
    "crates/velos-client",
    "crates/velos-log-engine",
    "crates/velos-metrics",
    "crates/velos-mcp",
    "crates/velos-api",
    "crates/velos-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/user/velos"

[workspace.dependencies]
# Shared dependencies
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
toml = "0.8"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
axum = "0.8"
ratatui = "0.29"
```

### Граф зависимостей crates

```
velos-cli (binary)
├── velos-core
├── velos-client
│   ├── velos-core
│   └── velos-ffi (optional, for embedded daemon mode)
├── velos-log-engine
│   └── velos-core
├── velos-metrics
│   └── velos-core
├── velos-mcp
│   ├── velos-core
│   ├── velos-client
│   └── velos-log-engine
└── velos-api
    ├── velos-core
    ├── velos-client
    └── velos-log-engine

velos-ffi (FFI layer)
└── links: libvelos_core.a (Zig static library)
```

### Build pipeline

```
1. zig build -Doptimize=ReleaseFast
   └─> zig/zig-out/lib/libvelos_core.a
   └─> include/velos_core.h

2. cargo build --release
   └─> crates/velos-ffi/build.rs обнаруживает libvelos_core.a
   └─> линкует через #[link(name = "velos_core")]
   └─> target/release/velos (single binary)
```

### Директория runtime (~/.velos/)

```
~/.velos/
├── velos.sock              # Unix domain socket (IPC)
├── velos.pid               # Daemon PID file
├── config.toml             # Global configuration
├── state.json              # Saved process list (velos save)
├── logs/
│   ├── api-out.log         # stdout для процесса "api"
│   ├── api-err.log         # stderr для процесса "api"
│   ├── worker-out.log
│   ├── worker-err.log
│   └── daemon.log          # Логи самого daemon'а
└── metrics/
    └── history.db          # SQLite для исторических метрик (опционально)
```

---

## 3. IPC протокол (CLI <-> Daemon)

### 3.1 Транспорт

| Платформа | Транспорт | Путь |
|---|---|---|
| Linux/macOS | Unix domain socket (SOCK_STREAM) | `~/.velos/velos.sock` |
| Windows | Named pipe | `\\.\pipe\velos` |

### 3.2 Формат сообщений (бинарный)

Кастомный бинарный протокол для минимального overhead. Без JSON на уровне IPC — это не HTTP API, а внутренний канал.

```
┌──────────────────────────────────────────────────┐
│                  Message Frame                    │
├──────────┬──────────┬──────────┬─────────────────┤
│ magic    │ version  │ length   │ payload         │
│ (2 bytes)│ (1 byte) │ (4 bytes)│ (variable)      │
│ 0xVE 0x10│ 0x01     │ LE u32   │ msgpack encoded │
├──────────┴──────────┴──────────┴─────────────────┤
│ Total header: 7 bytes                            │
└──────────────────────────────────────────────────┘
```

- **magic**: `0xVE10` — идентификатор протокола Velos
- **version**: протокол версия (1)
- **length**: длина payload в bytes (little-endian u32, max 16MB)
- **payload**: MessagePack encoded данные (компактнее JSON, быстрее парсить)

### 3.3 Схема команд (Request/Response)

```
Request {
    id: u32,              // Уникальный ID запроса (для мультиплексирования)
    command: u8,          // Код команды (см. таблицу ниже)
    payload: bytes        // Данные команды (зависят от command)
}

Response {
    id: u32,              // Соответствует request.id
    status: u8,           // 0 = ok, 1 = error, 2 = streaming
    payload: bytes        // Данные ответа
}
```

### 3.4 Команды

| Code | Команда | Payload (Request) | Payload (Response) |
|------|---------|--------------------|--------------------|
| 0x01 | PROCESS_START | ProcessConfig | { id: u32, pid: u32 } |
| 0x02 | PROCESS_STOP | { id: u32, signal: i32, timeout: u32 } | { success: bool } |
| 0x03 | PROCESS_RESTART | { id: u32 } | { pid: u32 } |
| 0x04 | PROCESS_DELETE | { id: u32 } | { success: bool } |
| 0x05 | PROCESS_LIST | {} | [ProcessInfo, ...] |
| 0x06 | PROCESS_INFO | { id: u32 } | ProcessInfo |
| 0x07 | PROCESS_RELOAD | { id: u32 } | { success: bool } |
| 0x10 | LOG_READ | { id: u32, lines: u32, level: u8?, grep: str? } | [LogEntry, ...] |
| 0x11 | LOG_STREAM | { id: u32, level: u8? } | STREAMING [LogEntry, ...] |
| 0x12 | LOG_SEARCH | { id: u32, pattern: str, since: u64?, until: u64? } | [LogEntry, ...] |
| 0x20 | METRICS_GET | { id: u32 } | Metrics |
| 0x21 | METRICS_STREAM | { interval_ms: u32 } | STREAMING [Metrics, ...] |
| 0x30 | STATE_SAVE | {} | { path: str } |
| 0x31 | STATE_LOAD | { path: str? } | { count: u32 } |
| 0x40 | DAEMON_PING | {} | { version: str, uptime: u64 } |
| 0x41 | DAEMON_SHUTDOWN | {} | { success: bool } |
| 0xFF | ERROR | — | { code: u16, message: str } |

### 3.5 Streaming механизм

Для команд LOG_STREAM и METRICS_STREAM daemon отправляет непрерывный поток Response с `status: 2 (streaming)`. Клиент закрывает стрим отправкой:

```
Request { id: <original_id>, command: 0xFE (CANCEL_STREAM), payload: {} }
```

### 3.6 Конкурентность

- Daemon использует **epoll/kqueue** event loop (Zig) для мультиплексирования клиентов
- Каждый клиент может отправлять несколько запросов параллельно (мультиплексирование по `id`)
- Daemon обрабатывает команды **последовательно в рамках одного процесса**, параллельно между процессами

---

## 4. Модель данных процессов и конфигурации

### 4.1 ProcessConfig (из TOML)

```rust
/// Конфигурация одного процесса (парсится из TOML)
struct ProcessConfig {
    name: String,
    script: String,                    // путь к скрипту / бинарнику
    cwd: Option<String>,               // рабочая директория (default: dirname(script))
    interpreter: Option<String>,       // "node", "python3", None = auto-detect
    args: Vec<String>,                 // аргументы для скрипта
    instances: u32,                    // 1 = fork mode, >1 = cluster mode
    mode: ProcessMode,                 // Fork | Cluster
    env: HashMap<String, String>,      // переменные окружения
    env_profiles: HashMap<String, HashMap<String, String>>,  // env.staging, env.production

    // Restart policy
    autorestart: bool,                 // default: true
    max_restarts: Option<u32>,         // None = unlimited
    min_uptime: Duration,              // default: 1s (don't count as stable if less)
    restart_delay: Duration,           // default: 0ms
    exp_backoff_restart_delay: Option<Duration>,  // exponential backoff

    // Resource limits
    max_memory: Option<u64>,           // bytes, trigger restart if exceeded
    kill_timeout: Duration,            // default: 5s (SIGTERM -> wait -> SIGKILL)

    // Watch mode
    watch: bool,                       // default: false
    watch_paths: Vec<String>,          // default: ["."]
    watch_ignore: Vec<String>,         // default: ["node_modules", ".git", "*.log"]
    watch_delay: Duration,             // debounce, default: 1s

    // Scheduling
    cron_restart: Option<String>,      // cron expression for periodic restarts

    // Logging
    log_file: Option<String>,          // custom log path
    log_date_format: Option<String>,   // timestamp format
    merge_logs: bool,                  // merge stdout + stderr into one file

    // Advanced
    wait_ready: bool,                  // wait for process.send('ready') signal
    listen_timeout: Duration,          // default: 8s (for wait_ready)
    shutdown_with_message: bool,       // send {type:'shutdown'} via IPC instead of signal
}

enum ProcessMode {
    Fork,
    Cluster,
}
```

### 4.2 ProcessState (runtime)

```rust
/// Состояние процесса в runtime (хранится в daemon)
struct ProcessState {
    id: u32,                           // уникальный ID (auto-increment)
    config: ProcessConfig,             // исходная конфигурация
    status: ProcessStatus,
    instances: Vec<InstanceState>,     // для cluster mode — несколько инстансов
    created_at: Timestamp,
    updated_at: Timestamp,
}

struct InstanceState {
    instance_id: u32,                  // 0-based (NODE_APP_INSTANCE)
    pid: Option<u32>,                  // None if stopped
    status: ProcessStatus,
    cpu_percent: f64,
    memory_bytes: u64,
    memory_peak_bytes: u64,
    started_at: Option<Timestamp>,
    restart_count: u32,
    last_exit_code: Option<i32>,
    last_exit_signal: Option<i32>,
    uptime: Duration,
}

enum ProcessStatus {
    Starting,                          // процесс запускается
    Online,                            // работает нормально
    Stopping,                          // получил SIGTERM, ожидание завершения
    Stopped,                           // остановлен пользователем
    Errored,                           // завершился с ошибкой
    WaitingRestart,                    // ожидает перезапуска (backoff)
}
```

### 4.3 State persistence (state.json)

```json
{
  "version": 1,
  "saved_at": "2026-02-12T10:30:00Z",
  "daemon_version": "0.1.0",
  "processes": [
    {
      "id": 1,
      "config": {
        "name": "api",
        "script": "./api.js",
        "instances": 4,
        "mode": "cluster",
        "env": {"NODE_ENV": "production"},
        "autorestart": true,
        "max_memory": 1073741824
      },
      "status": "online",
      "restart_count": 3,
      "created_at": "2026-02-12T08:00:00Z"
    }
  ]
}
```

### 4.4 Лог-файлы (structured JSON Lines)

Каждая строка лога — JSON object (для программного парсинга):

```jsonl
{"ts":1707734400000,"lvl":"info","pid":12345,"msg":"Server started on port 3000"}
{"ts":1707734401000,"lvl":"error","pid":12345,"msg":"ECONNREFUSED 127.0.0.1:5432","src":"stderr"}
{"ts":1707734402000,"lvl":"warn","pid":12345,"msg":"Memory usage high: 890MB/1GB"}
```

Поля:
- `ts` — timestamp (unix ms)
- `lvl` — level (auto-classified: debug/info/warn/error/fatal)
- `pid` — PID дочернего процесса
- `msg` — текст сообщения
- `src` — источник: "stdout" | "stderr"

При `--raw` режиме — запись как есть (plain text) для совместимости.

---

## 5. Smart Log Engine

### 5.1 Архитектура

```
Raw log stream (from daemon)
       │
       ▼
┌──────────────────┐
│  Auto-Classifier │  regex rules → assign level (info/warn/error)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Deduplicator    │  hash-based grouping → collapse repeats
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Pattern Detector │  frequency analysis → identify recurring patterns
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Anomaly Detector │  statistical → flag unusual spikes
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Summary Generator│  aggregate → compact report
└──────────────────┘
```

### 5.2 Auto-Classifier

Определяет уровень лога если приложение не выдаёт структурированный вывод.

```rust
struct ClassificationRule {
    pattern: Regex,
    level: LogLevel,
    priority: u8,    // higher = checked first
}

// Default rules (configurable via TOML):
const DEFAULT_RULES: &[(&str, LogLevel)] = &[
    (r"(?i)\b(fatal|panic|critical)\b", LogLevel::Fatal),
    (r"(?i)\b(error|err|exception|fail(ed|ure)?)\b", LogLevel::Error),
    (r"(?i)\b(warn(ing)?|deprecated)\b", LogLevel::Warn),
    (r"(?i)\b(debug|trace|verbose)\b", LogLevel::Debug),
    // Default: Info
];
```

### 5.3 Deduplicator

Группирует повторяющиеся сообщения в окне.

```rust
struct DedupEngine {
    /// hash(normalized_message) -> DedupEntry
    entries: HashMap<u64, DedupEntry>,
    window: Duration,            // default: 60s
    similarity_threshold: f64,   // 0.0-1.0, default: 0.85
}

struct DedupEntry {
    template: String,            // нормализованный шаблон
    count: u64,
    first_seen: Timestamp,
    last_seen: Timestamp,
    sample: String,              // первое оригинальное сообщение
}

// Нормализация: убираем числа, UUID, timestamps из строки
// "Connection to 192.168.1.5:5432 failed" → "Connection to <IP>:<PORT> failed"
// Это позволяет группировать похожие ошибки с разными параметрами
```

Алгоритм:
1. Нормализовать сообщение (заменить IP, числа, UUID на плейсхолдеры)
2. Вычислить hash нормализованной строки
3. Если hash есть в `entries` и `last_seen` в пределах `window` → increment count
4. Иначе → создать новый entry
5. При выводе: `"Connection to <IP>:<PORT> failed (x847, first: 10:05, last: 10:47)"`

### 5.4 Pattern Detector

```rust
struct PatternDetector {
    patterns: Vec<DetectedPattern>,
    min_frequency: u32,          // минимум повторений для детекции (default: 5)
    time_window: Duration,       // окно анализа (default: 5min)
}

struct DetectedPattern {
    template: String,
    frequency: u32,              // раз/минуту
    level: LogLevel,
    first_seen: Timestamp,
    last_seen: Timestamp,
    trend: Trend,                // Rising | Stable | Declining
}

enum Trend {
    Rising,      // частота растёт
    Stable,      // постоянная
    Declining,   // снижается
}
```

### 5.5 Anomaly Detector

Статистический — без ML/LLM.

```rust
struct AnomalyDetector {
    /// Скользящее окно метрик
    error_rate: SlidingWindow<f64>,      // ошибок/минуту
    log_volume: SlidingWindow<f64>,      // строк/минуту
    window_size: usize,                  // default: 60 (60 минут истории)
    sigma_threshold: f64,                // default: 2.0 (2 стандартных отклонения)
}

struct Anomaly {
    metric: String,              // "error_rate" | "log_volume"
    current_value: f64,
    mean: f64,
    std_dev: f64,
    sigma: f64,                  // сколько σ от среднего
    timestamp: Timestamp,
    severity: AnomalySeverity,   // Warning (2σ) | Critical (3σ)
}
```

Алгоритм:
1. Каждую минуту записываем error_rate и log_volume в sliding window
2. Вычисляем mean и std_dev за окно
3. Если текущее значение > mean + sigma_threshold * std_dev → anomaly
4. 2σ = Warning, 3σ = Critical

### 5.6 Summary Generator

```rust
struct LogSummary {
    process_name: String,
    period: (Timestamp, Timestamp),
    total_lines: u64,
    by_level: HashMap<LogLevel, u64>,   // {info: 5420, warn: 23, error: 7}
    top_patterns: Vec<PatternSummary>,  // Top-5 повторяющихся паттернов
    anomalies: Vec<Anomaly>,           // Активные аномалии
    last_error: Option<String>,        // Последняя ошибка (полный текст)
    last_restart: Option<Timestamp>,   // Когда последний раз рестартовал
    health_score: u8,                  // 0-100 (эвристика)
}
```

Пример вывода `velos logs api --summary`:
```
Process: api | Period: last 1h | Health: 87/100
Lines: 5,450 | Errors: 7 | Warnings: 23
Top patterns:
  1. "Connection to <IP>:<PORT> timeout" (x12, trend: rising)
  2. "Cache miss for key <KEY>" (x847, trend: stable)
Last error: "ECONNREFUSED 10.0.0.5:5432" (2min ago)
Anomaly: error_rate 3.2σ above normal (7/min vs avg 0.5/min)
```

---

## 6. MCP Server и AI-интерфейс

### 6.1 MCP Tools

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `process_list` | Список всех процессов | `{}` | `[{name, status, cpu, mem, uptime}]` |
| `process_start` | Запустить процесс | `{script, name?, instances?, env?}` | `{id, name, pid, status}` |
| `process_stop` | Остановить процесс | `{name_or_id}` | `{success, message}` |
| `process_restart` | Перезапустить | `{name_or_id}` | `{success, pid}` |
| `process_delete` | Удалить процесс | `{name_or_id}` | `{success}` |
| `process_info` | Детальная информация | `{name_or_id}` | `{config, state, metrics}` |
| `log_read` | Последние N строк | `{name_or_id, lines?, level?}` | `[{ts, lvl, msg}]` |
| `log_search` | Поиск по логам | `{name_or_id, pattern, since?, until?, level?}` | `[{ts, lvl, msg}]` |
| `log_summary` | Сводка логов (экономия токенов) | `{name_or_id, period?}` | `LogSummary` |
| `health_check` | Здоровье всех процессов | `{}` | `{overall, processes: [{name, score, issues}]}` |
| `config_get` | Текущая конфигурация | `{name_or_id}` | `ProcessConfig (TOML)` |
| `config_set` | Изменить конфигурацию | `{name_or_id, changes}` | `{success, applied}` |
| `metrics_snapshot` | Текущие метрики | `{name_or_id?}` | `{cpu, mem, restarts, uptime}` |

### 6.2 Принцип экономии токенов

Каждый MCP tool возвращает **минимально необходимые данные**:

```
# Плохо (как делают другие): 2847 токенов
{"processes":[{"id":1,"name":"api","namespace":"default","pm_id":0,
"pm2_env":{"status":"online","pm_uptime":1707734400000,...50 полей...},...}]}

# Velos MCP: 89 токенов
[{"name":"api","status":"online","cpu":2.1,"mem":"45M","up":"2d","restarts":0}]
```

### 6.3 --ai флаг CLI

Каждая CLI команда поддерживает `--ai` для компактного вывода:

```bash
# velos list --ai
[{"n":"api","s":"on","c":2.1,"m":"45M","u":"2d"},{"n":"worker","s":"on","c":0.3,"m":"12M","u":"2d"}]

# velos logs api --ai --last 5m --level error
[{"t":1707734401,"m":"ECONNREFUSED 127.0.0.1:5432"},{"t":1707734405,"m":"Timeout after 30s"}]
```

Ключи сокращены: `n`=name, `s`=status, `c`=cpu, `m`=memory, `u`=uptime, `t`=timestamp, `m`=message

---

## 7. Система метрик и observability

### 7.1 Собираемые метрики

**Per-process:**
| Метрика | Тип | Описание |
|---------|-----|----------|
| `velos_process_cpu_percent` | gauge | CPU% процесса |
| `velos_process_memory_bytes` | gauge | RSS памяти |
| `velos_process_memory_peak_bytes` | gauge | Пиковое потребление |
| `velos_process_uptime_seconds` | gauge | Время работы |
| `velos_process_restart_total` | counter | Общее число рестартов |
| `velos_process_status` | gauge | Статус (0=stopped, 1=online, 2=errored) |
| `velos_process_log_lines_total` | counter | Всего строк лога |
| `velos_process_log_errors_total` | counter | Ошибок в логах |

**Daemon-level:**
| Метрика | Тип | Описание |
|---------|-----|----------|
| `velos_daemon_uptime_seconds` | gauge | Uptime daemon'а |
| `velos_daemon_processes_total` | gauge | Число управляемых процессов |
| `velos_daemon_memory_bytes` | gauge | RAM daemon'а (<2MB цель) |
| `velos_daemon_ipc_requests_total` | counter | IPC запросов обработано |
| `velos_daemon_ipc_latency_seconds` | histogram | Латентность IPC |

### 7.2 Prometheus endpoint

```bash
velos metrics --port 9615
# GET http://localhost:9615/metrics
```

Формат:
```
# HELP velos_process_cpu_percent CPU usage percentage
# TYPE velos_process_cpu_percent gauge
velos_process_cpu_percent{name="api",instance="0"} 2.15
velos_process_cpu_percent{name="api",instance="1"} 1.87
velos_process_memory_bytes{name="api",instance="0"} 47185920
velos_process_restart_total{name="api",instance="0"} 3
velos_daemon_memory_bytes 1572864
```

### 7.3 OpenTelemetry

Трассировка для каждого процесса:

```
Span: velos.process.lifecycle
├── name: "api"
├── status: "online"
├── attributes:
│   ├── process.pid: 12345
│   ├── process.restart_count: 3
│   ├── process.memory_bytes: 47185920
│   └── process.cpu_percent: 2.15
└── events:
    ├── "process.started" (timestamp)
    ├── "process.restart" (timestamp, reason: "crash")
    └── "process.error" (timestamp, message: "ECONNREFUSED")
```

Resource attributes:
```
service.name: "velos"
service.version: "0.1.0"
host.name: "prod-server-01"
```

### 7.4 TUI Dashboard (velos monit)

```
┌─ Velos Monitor ──────────────────────────────────────────────────┐
│ Daemon: v0.1.0 | Uptime: 14d 3h | RAM: 1.2MB | Processes: 5    │
├──────────────────────────────────────────────────────────────────┤
│ Name       │ Status  │ CPU    │ Memory  │ Uptime  │ Restarts   │
│────────────│─────────│────────│─────────│─────────│────────────│
│ api:0      │ online  │ 2.1%   │ 45 MB   │ 2d 3h   │ 0          │
│ api:1      │ online  │ 1.8%   │ 43 MB   │ 2d 3h   │ 0          │
│ api:2      │ online  │ 2.3%   │ 47 MB   │ 2d 3h   │ 1          │
│ api:3      │ online  │ 1.5%   │ 41 MB   │ 2d 3h   │ 0          │
│ worker     │ online  │ 0.3%   │ 12 MB   │ 2d 3h   │ 0          │
├──────────────────────────────────────────────────────────────────┤
│ [CPU ████░░░░░░ 8%]  [RAM ██████░░░░ 188MB/8GB]                │
├──────────────────────────────────────────────────────────────────┤
│ Recent logs (api):                                               │
│ 10:05:03 [INFO] Request handled in 23ms                          │
│ 10:05:04 [WARN] Slow query: SELECT * FROM users (250ms)         │
│ 10:05:05 [INFO] Health check passed                              │
└──────────────────────────────────────────────────────────────────┘
  [q]uit  [↑↓]select  [l]ogs  [r]estart  [s]top  [d]elete
```

---

## 8. Build System и CI/CD

### 8.1 Build pipeline (детальный)

```
┌────────────────────────────────────────────────────────┐
│                    make build                          │
├────────────────────────────────────────────────────────┤
│                                                        │
│  Step 1: Zig build                                     │
│  ┌──────────────────────────────────────┐              │
│  │ cd zig && zig build -Doptimize=...   │              │
│  │                                       │              │
│  │ Input:  zig/src/**/*.zig              │              │
│  │ Output: zig/zig-out/lib/libvelos_core.a             │
│  │         include/velos_core.h          │              │
│  └──────────────────┬───────────────────┘              │
│                     │                                   │
│  Step 2: Rust build (cargo)                            │
│  ┌──────────────────▼───────────────────┐              │
│  │ cargo build --release                 │              │
│  │                                       │              │
│  │ crates/velos-ffi/build.rs:            │              │
│  │   1. Detect zig/zig-out/lib/          │              │
│  │   2. println!("cargo:rustc-link-      │              │
│  │      search=zig/zig-out/lib")         │              │
│  │   3. println!("cargo:rustc-link-      │              │
│  │      lib=static=velos_core")          │              │
│  │                                       │              │
│  │ Output: target/release/velos          │              │
│  └───────────────────────────────────────┘              │
└────────────────────────────────────────────────────────┘
```

### 8.2 Makefile

```makefile
.PHONY: build build-zig build-rust clean test release

# Default target
build: build-zig build-rust

# Zig core library
build-zig:
	cd zig && zig build -Doptimize=ReleaseFast

build-zig-debug:
	cd zig && zig build

# Rust workspace
build-rust:
	cargo build --release

build-rust-debug:
	cargo build

# Development (debug, fast iteration)
dev: build-zig-debug
	cargo build

# Run all tests
test: build-zig-debug
	cd zig && zig build test
	cargo test --workspace

# Clean everything
clean:
	cd zig && zig build --clean
	cargo clean
	rm -rf include/velos_core.h

# Cross-compile releases
release-linux-x86:
	cd zig && zig build -Doptimize=ReleaseFast -Dtarget=x86_64-linux-gnu
	cross build --release --target x86_64-unknown-linux-gnu

release-linux-arm:
	cd zig && zig build -Doptimize=ReleaseFast -Dtarget=aarch64-linux-gnu
	cross build --release --target aarch64-unknown-linux-gnu

release-macos-x86:
	cd zig && zig build -Doptimize=ReleaseFast -Dtarget=x86_64-macos
	cargo build --release --target x86_64-apple-darwin

release-macos-arm:
	cd zig && zig build -Doptimize=ReleaseFast -Dtarget=aarch64-macos
	cargo build --release --target aarch64-apple-darwin

release-windows:
	cd zig && zig build -Doptimize=ReleaseFast -Dtarget=x86_64-windows-gnu
	cross build --release --target x86_64-pc-windows-gnu

# Install locally
install: build
	cp target/release/velos /usr/local/bin/velos
```

### 8.3 Cross-compilation matrix

| Target | Zig Target | Rust Target | Binary |
|--------|-----------|-------------|--------|
| Linux x86_64 | x86_64-linux-gnu | x86_64-unknown-linux-gnu | velos |
| Linux ARM64 | aarch64-linux-gnu | aarch64-unknown-linux-gnu | velos |
| macOS x86_64 | x86_64-macos | x86_64-apple-darwin | velos |
| macOS ARM64 | aarch64-macos | aarch64-apple-darwin | velos |
| Windows x86_64 | x86_64-windows-gnu | x86_64-pc-windows-gnu | velos.exe |

Zig как кросс-компилятор решает проблему cross-compilation: он умеет собирать для любой платформы без дополнительных toolchains.

### 8.4 CI/CD (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: goto-bus-stop/setup-zig@v2
      - uses: dtolnay/rust-toolchain@stable
      - run: make test

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy, rustfmt }
      - run: cargo fmt --check
      - run: cargo clippy --workspace -- -D warnings
```

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            zig-target: x86_64-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            zig-target: aarch64-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
            zig-target: x86_64-macos
          - os: macos-latest
            target: aarch64-apple-darwin
            zig-target: aarch64-macos
          - os: windows-latest
            target: x86_64-pc-windows-gnu
            zig-target: x86_64-windows-gnu
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: goto-bus-stop/setup-zig@v2
      - uses: dtolnay/rust-toolchain@stable
      - run: cd zig && zig build -Doptimize=ReleaseFast -Dtarget=${{ matrix.zig-target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v2
        with:
          files: target/${{ matrix.target }}/release/velos*

  publish-crate:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo publish -p velos-core
      - run: cargo publish -p velos-cli
```

### 8.5 Дистрибуция

| Канал | Команда установки |
|-------|-------------------|
| Cargo | `cargo install velos` |
| Homebrew | `brew install velos` |
| npm (wrapper) | `npm install -g velos` (опционально, для обратной совместимости) |
| Binary | Скачать с GitHub Releases |
| Docker | `docker pull ghcr.io/user/velos` |
| AUR (Arch) | `yay -S velos` |
