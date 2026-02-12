# Phase 5: AI Features + MCP Server

> **Статус:** Not started
> **Зависимости:** Phase 4
> **Цель:** AI-native интеграция. MCP Server, token-efficient вывод.
> **Результат:** `velos mcp-server`, --ai флаг, AI-агенты управляют процессами.

---

## 5.1 AI-friendly CLI вывод

- [ ] `--ai` флаг для всех команд — компактный JSON с сокращёнными ключами
  - [ ] `velos list --ai` → `[{"n":"api","s":"on","c":2.1,"m":"45M","u":"2d"}]`
  - [ ] `velos logs <name> --ai` → `[{"t":...,"l":"err","m":"..."}]`
  - [ ] `velos info <name> --ai` → компактная сводка
- [ ] `--json` флаг — полный JSON (без сокращений, для программного использования)
- [ ] `--no-color` / `--plain` — отключение цветов и таблиц (для pipe)
- [ ] Определить маппинг полных ключей → сокращённых для --ai режима

## 5.2 MCP Server — инфраструктура

- [ ] Создать `velos-mcp` crate
- [ ] MCP stdio transport (JSON-RPC через stdin/stdout)
- [ ] `velos mcp-server` CLI команда (запуск MCP сервера)
- [ ] JSON Schema для input/output каждого tool
- [ ] Error handling: MCP error responses с понятными сообщениями

## 5.3 MCP Tools — процессы

- [ ] `process_list` — список процессов (компактный формат)
- [ ] `process_start` — запуск процесса (script, name, instances, env)
- [ ] `process_stop` — остановка (по имени или ID)
- [ ] `process_restart` — перезапуск
- [ ] `process_delete` — удаление
- [ ] `process_info` — детальная информация (config + state + metrics)

## 5.4 MCP Tools — логи

- [ ] `log_read` — последние N строк (с фильтрацией по level)
- [ ] `log_search` — поиск по regex + time range
- [ ] `log_summary` — сводка логов (ключевой tool для экономии токенов)

## 5.5 MCP Tools — мониторинг и конфигурация

- [ ] `health_check` — здоровье всех процессов (overall score + per-process)
- [ ] `metrics_snapshot` — текущие метрики (CPU, RAM, restarts, uptime)
- [ ] `config_get` — текущая конфигурация процесса
- [ ] `config_set` — изменить конфигурацию (env vars, restart policy, etc.)

## 5.6 Документация MCP

- [ ] `docs/mcp-tools.md` — reference документация всех tools
- [ ] Примеры использования с Claude Code, Cursor, VS Code
- [ ] Пример конфигурации для `claude_desktop_config.json`

---

## Заметки

- MCP Server использует тот же velos-client для общения с daemon (не дублирует логику)
- log_summary — самый важный tool: вместо 50000 строк логов AI получает 10 строк сводки
- Для MCP SDK использовать: rmcp или собственная реализация JSON-RPC поверх stdio
- Ключи в --ai режиме: n=name, s=status, c=cpu, m=memory, u=uptime, t=timestamp, l=level
