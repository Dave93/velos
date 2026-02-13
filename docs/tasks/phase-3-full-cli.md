# Phase 3: Full CLI + Process Management

> **Статус:** Done
> **Зависимости:** Phase 2
> **Цель:** Полнофункциональное управление процессами на уровне PM2.
> **Результат:** TOML config, авторестарт, watch mode, save/resurrect, CPU/RAM метрики.

---

## 3.1 Конфигурация (TOML)

- [x] `velos-config` crate: парсинг TOML → ProcessConfig (serde + toml)
- [x] Валидация конфигурации (обязательные поля, типы, лимиты)
- [x] `velos start --config velos.toml` — запуск всех процессов из файла
- [x] `velos start --config velos.toml --only api` — запуск конкретного процесса из файла
- [x] Поддержка env profiles: `velos start --config velos.toml --env production`
- [x] Merge CLI аргументов с TOML конфигурацией (CLI приоритетнее)
- [x] Создать `config/velos.example.toml` с полным примером

## 3.2 Авторестарт

- [x] Daemon: при SIGCHLD + exit code != 0 → автоматический перезапуск
- [x] `autorestart: true/false` — конфигурация
- [x] `max_restarts` — лимит перезапусков (default: 15)
- [x] `min_uptime` — если процесс жил < min_uptime, считается crash loop
- [x] `restart_delay` — пауза перед перезапуском
- [x] `exp_backoff_restart_delay` — экспоненциальный backoff (100ms → 200ms → 400ms...)
- [x] Crash loop detection: если max_restarts исчерпан → статус "errored", не перезапускать

## 3.3 Resource Limits

- [x] `max_memory_restart` — мониторинг RSS, перезапуск при превышении
- [x] Zig: периодический polling CPU/RAM через syscalls (каждые 2s)
  - [x] Linux: чтение `/proc/[pid]/statm` для RSS
  - [x] macOS: `proc_pid_rusage()` для resident_size
- [x] Отображение CPU% и Memory в `velos list`

## 3.4 Дополнительные CLI команды

- [x] `velos restart <name|id|all>` — перезапуск
- [x] `velos reload <name|id|all>` — graceful reload (stop + start)
- [x] `velos info <name|id>` — детальная информация (config, state, metrics, restarts history)
- [x] `velos list --json` — JSON вывод (полный)

## 3.5 Graceful Shutdown

- [x] Configurable `kill_timeout` (default: 5s)
- [x] Последовательность: SIGTERM → wait(kill_timeout) → SIGKILL
- [x] `wait_ready` + IPC channel (Unix socketpair, VELOS_IPC_FD env var)
- [x] `shutdown_with_message` — отправка JSON `{type: "shutdown"}` через IPC channel

## 3.6 Watch Mode

- [x] File watcher: kqueue EVFILT_VNODE (macOS) / inotify (Linux)
- [x] `velos start app.js --watch` — перезапуск при изменении файлов
- [x] `watch_paths` — список путей для наблюдения (semicolon-separated)
- [x] `watch_ignore` — паттерны исключений (node_modules, .git, *.log)
- [x] Debounce: собирать изменения за `watch_delay` (default: 1s) перед рестартом

## 3.7 Interpreter Auto-Detection

- [x] По расширению: .js/.mjs/.cjs → node, .ts → npx tsx, .py → python3, .rb → ruby
- [x] По shebang: `#!/usr/bin/env node` → node
- [x] Без расширения / ELF/Mach-O → запуск напрямую как бинарник
- [x] Override через `interpreter` в конфигурации

## 3.8 State Persistence

- [x] `velos save` → записать текущий список процессов в `~/.velos/state.bin`
- [x] `velos resurrect` → загрузить из `~/.velos/state.bin`, запустить все процессы
- [x] Auto-save при каждом start/stop (опционально)

## 3.9 Log Rotation

- [x] Встроенная ротация по размеру файла (default: 10MB)
- [x] Configurable: `log_max_size`, `log_retain_count` (default: 30)
- [x] `velos flush [name]` — очистить логи

## 3.10 Cron Restart

- [x] Парсинг cron выражений (`cron_restart: "0 3 * * *"`)
- [x] Daemon: таймер для проверки cron расписания (localtime, once per minute)
- [x] Перезапуск по расписанию (полезно для long-running процессов с memory leak)

---

## Заметки

- Cluster mode всё ещё не реализуется (Phase 6)
- Windows support — пока low priority, но interpreter detection должен работать кросс-платформенно
- Watch mode использует kqueue (macOS) / inotify (Linux) — не FSEvents
- IPC channel для wait_ready/shutdown_with_message использует Unix socketpair + VELOS_IPC_FD env var
- Auto-save не выполняется при delete (осознанное удаление не должно сохраняться в state)
