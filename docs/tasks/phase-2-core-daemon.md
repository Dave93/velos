# Phase 2: Core Daemon + Basic CLI

> **Статус:** Not started
> **Зависимости:** Phase 1
> **Цель:** Daemon запускает/останавливает процессы. CLI общается с daemon через IPC.
> **Результат:** `velos start app.js` → процесс работает, `velos list` показывает его.

---

## 2.1 Daemon — Event Loop (Zig)

- [ ] Реализовать Platform Abstraction Layer (PAL) интерфейс
- [ ] Linux: epoll event loop
- [ ] macOS: kqueue event loop
- [ ] Main daemon loop: принимать IPC соединения + обрабатывать сигналы
- [ ] Daemonization: fork + setsid (или запуск в foreground с --no-daemon)

## 2.2 Daemon — IPC Server (Zig)

- [ ] Unix domain socket listener (bind, listen, accept)
- [ ] Бинарный протокол: header (magic + version + length) + MessagePack payload
- [ ] Мультиплексирование клиентов через event loop
- [ ] Обработка команд: dispatch по command code
- [ ] Отправка ответов (Response с id, status, payload)

## 2.3 Daemon — Process Supervisor (Zig)

- [ ] `fork()` + `exec()` для запуска дочерних процессов
- [ ] Создание pipe для stdout/stderr перехвата
- [ ] SIGCHLD handler → `waitpid()` → обнаружение завершения child
- [ ] Хранение PID map: process_id → pid, status, config
- [ ] Отправка сигналов дочерним процессам (SIGTERM, SIGKILL)
- [ ] Graceful stop: SIGTERM → wait(kill_timeout) → SIGKILL

## 2.4 Daemon — Log Collector (Zig)

- [ ] Чтение из pipe (stdout/stderr) дочерних процессов
- [ ] Запись в лог-файлы (`~/.velos/logs/<name>-out.log`, `<name>-err.log`)
- [ ] Ring buffer в памяти для последних 1000 строк (быстрый доступ)

## 2.5 Daemon — State & Lifecycle (Zig)

- [ ] PID file: запись `~/.velos/velos.pid` при старте, удаление при shutdown
- [ ] Signal handler: SIGTERM → graceful shutdown (остановить всех children)
- [ ] Создание директории `~/.velos/` и поддиректорий при первом запуске

## 2.6 Rust — IPC Client

- [ ] `velos-client`: подключение к Unix socket
- [ ] Отправка Request, получение Response
- [ ] Таймауты и обработка ошибок (daemon не запущен, connection refused)
- [ ] `velos-core/protocol.rs`: Rust типы для IPC сообщений (serde + rmp-serde)

## 2.7 CLI — Команды

- [ ] `velos start <script> [--name <name>]` → PROCESS_START
- [ ] `velos stop <name|id>` → PROCESS_STOP
- [ ] `velos list` → PROCESS_LIST → table output (name, PID, status, uptime)
- [ ] `velos logs <name> [--lines N]` → LOG_READ
- [ ] `velos delete <name|id>` → PROCESS_DELETE

## 2.8 Daemon Auto-Start

- [ ] При `velos start ...` CLI проверяет: daemon запущен? (проверка PID file + socket)
- [ ] Если нет → запускает daemon в фоне (fork + exec velos-daemon binary)
- [ ] Ожидание готовности daemon'а (retry connect с backoff, max 5s)

## 2.9 Тестирование

- [ ] Unit-тесты Zig: IPC protocol serialize/deserialize
- [ ] Unit-тесты Rust: velos-client connection
- [ ] Integration test: start → list → logs → stop → delete lifecycle
- [ ] Test: daemon auto-start и auto-detect

---

## Заметки

- На этом этапе Windows пока не поддерживается (только Linux + macOS)
- Cluster mode не реализуется — только fork mode (1 процесс на 1 запись)
- Авторестарт пока не реализуется — будет в Phase 3
- Interpreter detection (node, python) пока не реализуется — скрипт запускается напрямую
