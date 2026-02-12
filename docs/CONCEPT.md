# Velos — Концепция проекта

## Позиционирование

**Velos** — высокопроизводительный, AI-friendly процесс-менеджер нового поколения.
Zig-ядро + Rust-оболочка. Кросс-платформенный (Linux, macOS, Windows).
Первый process manager с нативным MCP-сервером и алгоритмической AI-оптимизацией логов.

**Целевая аудитория:** Разработчики + DevOps/SRE

---

## Архитектура (Zig + Rust гибрид)

### Zig-ядро (libvelos)

- Daemon-процесс с минимальным footprint (<2MB RAM)
- Управление процессами: fork/exec, сигналы, reaping
- IPC через Unix sockets / Named pipes (Windows)
- Мониторинг: прямые syscalls для CPU/RAM (без /proc парсинга где возможно)
- Custom allocator для предотвращения утечек памяти (главная боль PM2)
- Опциональная интеграция с systemd/launchd для надёжности

### Rust-оболочка

- CLI (clap) — красивый, быстрый интерфейс
- REST API + WebSocket (axum/tokio)
- MCP Server (встроенный)
- TOML парсинг конфигурации
- Prometheus exporter / OpenTelemetry SDK
- Система плагинов
- AI-friendly log engine

### Связь Zig <-> Rust

Через C ABI: Zig -> static library -> Rust FFI

---

## Полный набор фичей для v0.1

### 1. Управление процессами (Core)

- `velos start <script>` — запуск (поддержка Node.js, Python, Go, Rust, любой бинарник)
- `velos stop/restart/delete <name|id>`
- `velos list` — таблица процессов с CPU/RAM/uptime
- `velos reload` — zero-downtime reload
- Авторестарт при падении с настраиваемой стратегией
- `max_memory_restart` — лимит по памяти
- Cron-расписание перезапусков
- Graceful shutdown (SIGTERM -> wait -> SIGKILL) с настраиваемым таймаутом
- `wait_ready` + сигнал готовности от приложения

### 2. Кластеризация

- `velos start app.js -i max` — кластерный режим
- Балансировка нагрузки между инстансами
- Rolling restart для zero-downtime обновлений

### 3. Watch Mode

- `velos start app.js --watch`
- Настраиваемые пути и ignore-паттерны
- Debounce для предотвращения каскадных рестартов

### 4. Конфигурация (TOML)

```toml
[process.api]
script = "./api.js"
instances = 4
mode = "cluster"
max_memory = "1G"
watch = true
watch_ignore = ["node_modules", ".git"]

[process.api.env]
NODE_ENV = "production"
PORT = "3000"

[process.api.env.staging]
NODE_ENV = "staging"
PORT = "3001"
```

### 5. Логирование

- Централизованное хранение в `~/.velos/logs/`
- Ротация логов (встроенная, без модулей)
- `velos logs <name>` — потоковый просмотр
- `velos logs <name> --lines 100` — последние N строк

### 6. Мониторинг

- `velos monit` — TUI-мониторинг в реальном времени
- CPU, RAM, event loop lag, request rate
- Prometheus endpoint: `velos metrics --port 9615`
- OpenTelemetry traces/spans для каждого процесса

### 7. Startup / Persistence

- `velos startup` — генерация systemd/launchd/init скрипта
- `velos save` — сохранение текущего состояния
- Автоматическое восстановление после перезагрузки

---

## AI-Friendly фичи (без расходов на LLM)

### 8. Структурированный вывод

```bash
# Обычный вывод для человека
velos list

# Компактный JSON для AI-агентов (минимум токенов)
velos list --ai
# {"processes":[{"name":"api","status":"online","cpu":2.1,"mem":"45M","uptime":"2d"}]}

# Полный JSON
velos list --json
```

Флаг `--ai` — специальный режим:

- Минимизация вывода (только ключевые данные)
- Убирает таблицы, цвета, рамки
- Компактный JSON без лишних полей
- Экономия до 80% токенов по сравнению с `--json`

### 9. Умные логи (Smart Log Engine)

```bash
# Поиск по паттерну — AI не грузит весь лог
velos logs api --grep "ERROR" --last "1h"

# Дедупликация — вместо 10000 одинаковых строк
velos logs api --dedupe
# [ERROR] Connection refused (x10847, first: 10:05:03, last: 10:47:22)

# Summary — компактная сводка состояния процесса
velos logs api --summary
# errors: 23, warnings: 156, last_error: "ECONNREFUSED at db.js:42",
# patterns: [{msg:"timeout", count:12}, {msg:"ECONNREFUSED", count:11}]

# Поиск по временному диапазону
velos logs api --since "2024-01-15 10:00" --until "2024-01-15 11:00"

# Фильтрация по уровню
velos logs api --level error,warn
```

Алгоритмические фичи (на уровне кода, 0 расходов):

- **Pattern detection** — группировка повторяющихся сообщений
- **Anomaly detection** — статистическое выявление аномальных паттернов (всплеск ошибок)
- **Auto-classification** — regex-классификация по уровням (error/warn/info/debug)
- **Compact digest** — автоматическая сводка за период

### 10. MCP Server (встроенный)

```bash
velos mcp-server  # запуск MCP-сервера через stdio
```

MCP Tools для AI-агентов:

- `process_list` — список процессов
- `process_start/stop/restart` — управление
- `process_logs` — с фильтрацией и дедупликацией
- `process_metrics` — текущие метрики
- `process_health_summary` — компактная сводка здоровья
- `log_search` — поиск по логам с regex
- `log_summary` — сводка логов (экономия токенов)
- `config_get/set` — чтение/изменение конфигурации

---

## Killer Features (сравнение с конкурентами)

| Фича | PM2 | PMDaemon | **Velos** |
|---|---|---|---|
| Язык ядра | Node.js | Rust | **Zig** (min footprint) |
| Daemon RAM | ~90MB+ | ~15MB | **<2MB** (цель) |
| MCP Server | Нет | Отдельный | **Встроенный** |
| AI-friendly вывод | Нет | Нет | **--ai флаг** |
| Log дедупликация | Нет | Нет | **Встроенная** |
| Log summary | Нет | Нет | **Алгоритмический** |
| Anomaly detection | Нет | Нет | **Статистический** |
| Prometheus | Платный (PM2+) | Нет | **Бесплатный** |
| OpenTelemetry | Нет | Нет | **Из коробки** |
| TOML config | Нет | Нет | **Нативный** |
| Systemd fallback | Нет | Нет | **Гибридный daemon** |
