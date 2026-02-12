# Phase 3: Full CLI + Process Management

> **Статус:** Not started
> **Зависимости:** Phase 2
> **Цель:** Полнофункциональное управление процессами на уровне PM2.
> **Результат:** TOML config, авторестарт, watch mode, save/resurrect, CPU/RAM метрики.

---

## 3.1 Конфигурация (TOML)

- [ ] `velos-config` crate: парсинг TOML → ProcessConfig (serde + toml)
- [ ] Валидация конфигурации (обязательные поля, типы, лимиты)
- [ ] `velos start --config velos.toml` — запуск всех процессов из файла
- [ ] `velos start --config velos.toml --only api` — запуск конкретного процесса из файла
- [ ] Поддержка env profiles: `velos start --config velos.toml --env production`
- [ ] Merge CLI аргументов с TOML конфигурацией (CLI приоритетнее)
- [ ] Создать `config/velos.example.toml` с полным примером

## 3.2 Авторестарт

- [ ] Daemon: при SIGCHLD + exit code != 0 → автоматический перезапуск
- [ ] `autorestart: true/false` — конфигурация
- [ ] `max_restarts` — лимит перезапусков (default: 15)
- [ ] `min_uptime` — если процесс жил < min_uptime, считается crash loop
- [ ] `restart_delay` — пауза перед перезапуском
- [ ] `exp_backoff_restart_delay` — экспоненциальный backoff (100ms → 200ms → 400ms...)
- [ ] Crash loop detection: если max_restarts исчерпан → статус "errored", не перезапускать

## 3.3 Resource Limits

- [ ] `max_memory_restart` — мониторинг RSS, перезапуск при превышении
- [ ] Zig: периодический polling CPU/RAM через syscalls (каждые 2s)
  - [ ] Linux: чтение `/proc/[pid]/stat` и `/proc/[pid]/status`
  - [ ] macOS: `proc_pid_rusage()` или `task_info()`
- [ ] Отображение CPU% и Memory в `velos list`

## 3.4 Дополнительные CLI команды

- [ ] `velos restart <name|id|all>` — перезапуск
- [ ] `velos reload <name|id|all>` — graceful reload (stop + start)
- [ ] `velos info <name|id>` — детальная информация (config, state, metrics, restarts history)
- [ ] `velos list --json` — JSON вывод (полный)

## 3.5 Graceful Shutdown

- [ ] Configurable `kill_timeout` (default: 5s)
- [ ] Последовательность: SIGTERM → wait(kill_timeout) → SIGKILL
- [ ] `wait_ready` + `process.send('ready')` (для Node.js IPC channel)
- [ ] `shutdown_with_message` — отправка JSON `{type: "shutdown"}` через IPC

## 3.6 Watch Mode

- [ ] File watcher: inotify (Linux) / FSEvents (macOS)
- [ ] `velos start app.js --watch` — перезапуск при изменении файлов
- [ ] `watch_paths` — список путей для наблюдения
- [ ] `watch_ignore` — glob-паттерны исключений (node_modules, .git, *.log)
- [ ] Debounce: собирать изменения за `watch_delay` (default: 1s) перед рестартом

## 3.7 Interpreter Auto-Detection

- [ ] По расширению: .js/.mjs/.cjs → node, .ts → npx tsx, .py → python3, .rb → ruby
- [ ] По shebang: `#!/usr/bin/env node` → node
- [ ] Без расширения / ELF/Mach-O → запуск напрямую как бинарник
- [ ] Override через `interpreter` в конфигурации

## 3.8 State Persistence

- [ ] `velos save` → записать текущий список процессов в `~/.velos/state.json`
- [ ] `velos resurrect` → загрузить из `~/.velos/state.json`, запустить все процессы
- [ ] Auto-save при каждом start/stop/delete (опционально)

## 3.9 Log Rotation

- [ ] Встроенная ротация по размеру файла (default: 10MB)
- [ ] Configurable: `log_max_size`, `log_retain_count` (default: 30)
- [ ] `velos flush [name]` — очистить логи

## 3.10 Cron Restart

- [ ] Парсинг cron выражений (`cron_restart: "0 3 * * *"`)
- [ ] Daemon: таймер для проверки cron расписания
- [ ] Перезапуск по расписанию (полезно для long-running процессов с memory leak)

---

## Заметки

- Cluster mode всё ещё не реализуется (Phase 6)
- Windows support — пока low priority, но interpreter detection должен работать кросс-платформенно
- Для cron парсинга можно использовать Zig-библиотеку или реализовать минимальный парсер
