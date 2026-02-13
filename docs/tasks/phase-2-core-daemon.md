# Phase 2: Core Daemon + Basic CLI

> **Статус:** Done
> **Зависимости:** Phase 1
> **Цель:** Daemon запускает/останавливает процессы. CLI общается с daemon через IPC.
> **Результат:** `velos start app.js` → процесс работает, `velos list` показывает его.

---

## 2.1 Daemon — Event Loop (Zig)

- [x] Реализовать Platform Abstraction Layer (PAL) интерфейс
- [ ] Linux: epoll event loop
- [x] macOS: kqueue event loop
- [x] Main daemon loop: принимать IPC соединения + обрабатывать сигналы
- [x] Daemonization: fork + setsid (или запуск в foreground с --no-daemon)

## 2.2 Daemon — IPC Server (Zig)

- [x] Unix domain socket listener (bind, listen, accept)
- [x] Бинарный протокол: header (magic + version + length) + MessagePack payload
- [x] Мультиплексирование клиентов через event loop
- [x] Обработка команд: dispatch по command code
- [x] Отправка ответов (Response с id, status, payload)

## 2.3 Daemon — Process Supervisor (Zig)

- [x] `fork()` + `exec()` для запуска дочерних процессов
- [x] Создание pipe для stdout/stderr перехвата
- [x] SIGCHLD handler → `waitpid()` → обнаружение завершения child
- [x] Хранение PID map: process_id → pid, status, config
- [x] Отправка сигналов дочерним процессам (SIGTERM, SIGKILL)
- [x] Graceful stop: SIGTERM → wait(kill_timeout) → SIGKILL

## 2.4 Daemon — Log Collector (Zig)

- [x] Чтение из pipe (stdout/stderr) дочерних процессов
- [x] Запись в лог-файлы (`~/.velos/logs/<name>-out.log`, `<name>-err.log`)
- [x] Ring buffer в памяти для последних 1000 строк (быстрый доступ)

## 2.5 Daemon — State & Lifecycle (Zig)

- [x] PID file: запись `~/.velos/velos.pid` при старте, удаление при shutdown
- [x] Signal handler: SIGTERM → graceful shutdown (остановить всех children)
- [x] Создание директории `~/.velos/` и поддиректорий при первом запуске

## 2.6 Rust — IPC Client

- [x] `velos-client`: подключение к Unix socket
- [x] Отправка Request, получение Response
- [x] Таймауты и обработка ошибок (daemon не запущен, connection refused)
- [x] `velos-core/protocol.rs`: Rust типы для IPC сообщений (serde + rmp-serde)

## 2.7 CLI — Команды

- [x] `velos start <script> [--name <name>]` → PROCESS_START
- [x] `velos stop <name|id>` → PROCESS_STOP
- [x] `velos list` → PROCESS_LIST → table output (name, PID, status, uptime)
- [x] `velos logs <name> [--lines N]` → LOG_READ
- [x] `velos delete <name|id>` → PROCESS_DELETE

## 2.8 Daemon Auto-Start

- [x] При `velos start ...` CLI проверяет: daemon запущен? (проверка PID file + socket)
- [x] Если нет → запускает daemon в фоне (fork + exec velos-daemon binary)
- [x] Ожидание готовности daemon'а (retry connect с backoff, max 5s)

## 2.9 Тестирование

- [x] Unit-тесты Zig: IPC protocol serialize/deserialize
- [x] Unit-тесты Rust: velos-client connection
- [x] Integration test: start → list → logs → stop → delete lifecycle
- [x] Test: daemon auto-start и auto-detect

---

## Заметки

- На этом этапе Windows пока не поддерживается (только Linux + macOS)
- Cluster mode не реализуется — только fork mode (1 процесс на 1 запись)
- Авторестарт пока не реализуется — будет в Phase 3
- Interpreter detection (node, python) пока не реализуется — скрипт запускается напрямую
