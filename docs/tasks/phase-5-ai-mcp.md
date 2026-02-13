# Phase 5: AI Features + MCP Server

> **Статус:** Done (24/25)
> **Зависимости:** Phase 4
> **Цель:** AI-native интеграция. MCP Server, token-efficient вывод.
> **Результат:** `velos mcp-server`, --ai флаг, AI-агенты управляют процессами.

---

## 5.1 AI-friendly CLI вывод

- [x] `--ai` флаг для всех команд — компактный JSON с сокращёнными ключами
  - [x] `velos list --ai` → `[{"n":"api","s":"on","c":2.1,"m":"45M","u":"2d"}]`
  - [x] `velos logs <name> --ai` → `[{"t":...,"l":"err","m":"..."}]`
  - [x] `velos info <name> --ai` → компактная сводка
- [x] `--json` флаг — полный JSON (без сокращений, для программного использования)
- [x] `--no-color` / `--plain` — [SKIP] Не нужно: clap поддерживает стандарт NO_COLOR env var
- [x] Определить маппинг полных ключей → сокращённых для --ai режима

## 5.2 MCP Server — инфраструктура

- [x] Создать `velos-mcp` crate
- [x] MCP stdio transport (JSON-RPC через stdin/stdout)
- [x] `velos mcp-server` CLI команда (запуск MCP сервера)
- [x] JSON Schema для input/output каждого tool
- [x] Error handling: MCP error responses с понятными сообщениями

## 5.3 MCP Tools — процессы

- [x] `process_list` — список процессов (компактный формат)
- [x] `process_start` — запуск процесса (script, name, instances, env)
- [x] `process_stop` — остановка (по имени или ID)
- [x] `process_restart` — перезапуск
- [x] `process_delete` — удаление
- [x] `process_info` — детальная информация (config + state + metrics)

## 5.4 MCP Tools — логи

- [x] `log_read` — последние N строк (с фильтрацией по level)
- [x] `log_search` — поиск по regex + time range
- [x] `log_summary` — сводка логов (ключевой tool для экономии токенов)

## 5.5 MCP Tools — мониторинг и конфигурация

- [x] `health_check` — здоровье всех процессов (overall score + per-process)
- [x] `metrics_snapshot` — текущие метрики (CPU, RAM, restarts, uptime)
- [x] `config_get` — текущая конфигурация процесса
- [ ] `config_set` — [BLOCKED] Требует поддержки live config changes в Zig daemon

## 5.6 Документация MCP

- [x] `docs/mcp-tools.md` — reference документация всех tools
- [x] Примеры использования с Claude Code, Cursor, VS Code
- [x] Пример конфигурации для `claude_desktop_config.json`

---

## Заметки

- MCP Server использует тот же velos-client для общения с daemon (не дублирует логику)
- log_summary — самый важный tool: вместо 50000 строк логов AI получает 10 строк сводки
- Собственная реализация JSON-RPC поверх stdio (без внешних MCP SDK)
- Ключи в --ai режиме: n=name, i=id, s=status, p=pid, m=memory, u=uptime, r=restarts, t=timestamp, l=level
