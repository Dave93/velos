# Velos — Трекер задач

> Обновляй чекбоксы `[ ]` → `[x]` по мере выполнения.
> Прогресс каждой фазы считается по чекбоксам внутри файлов.

## Общий прогресс: 17/254

| Фаза | Название | Файл | Прогресс | Статус |
|------|----------|------|----------|--------|
| 0 | Planning | [phase-0-planning.md](tasks/phase-0-planning.md) | 17/17 | Done |
| 1 | Skeleton + Build System | [phase-1-skeleton.md](tasks/phase-1-skeleton.md) | 0/22 | Not started |
| 2 | Core Daemon + Basic CLI | [phase-2-core-daemon.md](tasks/phase-2-core-daemon.md) | 0/38 | Not started |
| 3 | Full CLI + Process Mgmt | [phase-3-full-cli.md](tasks/phase-3-full-cli.md) | 0/43 | Not started |
| 4 | Smart Logs + Monitoring | [phase-4-smart-logs.md](tasks/phase-4-smart-logs.md) | 0/37 | Not started |
| 5 | AI Features + MCP | [phase-5-ai-mcp.md](tasks/phase-5-ai-mcp.md) | 0/25 | Not started |
| 6 | Clustering + Metrics | [phase-6-cluster-metrics.md](tasks/phase-6-cluster-metrics.md) | 0/32 | Not started |
| 7 | Polish + Release v0.1.0 | [phase-7-polish-release.md](tasks/phase-7-polish-release.md) | 0/40 | Not started |

## Граф зависимостей

```
Phase 0 (Done)
    |
Phase 1 (Skeleton)
    |
Phase 2 (Core Daemon)
    |
Phase 3 (Full CLI) -------+
    |                      |
    +--------+        Phase 6
Phase 4      |        (Cluster + Metrics)
(Smart Logs) |             |
    |        +-------------+
Phase 5                |
(AI + MCP)             |
    |                  |
    +------------------+
             |
        Phase 7
     (Polish + v0.1.0)
```

---

## Ключевые документы

| Документ | Описание |
|----------|----------|
| [CONCEPT.md](CONCEPT.md) | Видение, позиционирование, фичи |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Архитектура, протоколы, структуры данных |
| [ROADMAP.md](ROADMAP.md) | Описание фаз, цели, результаты |
| [TASKS.md](TASKS.md) | Этот файл — общий трекер |

---

## Как работать с задачами

1. Открой файл нужной фазы
2. Отметь задачу: `[ ]` → `[x]`
3. Если задача заблокирована — добавь `[BLOCKED]` и причину
4. Если задача не нужна — добавь `[SKIP]` и причину
5. Обнови прогресс в этом файле когда фаза завершена
