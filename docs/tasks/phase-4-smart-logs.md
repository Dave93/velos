# Phase 4: Smart Logs + Monitoring

> **Статус:** Not started
> **Зависимости:** Phase 3
> **Цель:** Smart Log Engine и TUI мониторинг.
> **Результат:** `velos logs --summary`, `velos logs --dedupe`, `velos monit` (TUI).

---

## 4.1 Log Engine — инфраструктура

- [ ] Создать `velos-log-engine` crate
- [ ] Определить trait `LogProcessor` (pipeline stage interface)
- [ ] Pipeline: raw stream → classifier → dedup → pattern → anomaly → output
- [ ] Конфигурация pipeline через TOML (включение/отключение стадий)

## 4.2 Auto-Classifier

- [ ] Regex-based level detection (error, warn, info, debug, fatal)
- [ ] Default ruleset (см. ARCHITECTURE.md секция 5.2)
- [ ] Custom rules из TOML конфигурации
- [ ] JSON-aware: если строка уже JSON с полем "level" → использовать его

## 4.3 Structured Logs

- [ ] Опциональный structured JSON Lines формат записи
- [ ] Каждая строка: `{"ts":..., "lvl":..., "pid":..., "msg":..., "src":...}`
- [ ] Fallback: plain text для совместимости (--raw)

## 4.4 Фильтрация логов (CLI)

- [ ] `velos logs <name> --grep <pattern>` — regex фильтрация
- [ ] `velos logs <name> --level error,warn` — фильтр по уровню
- [ ] `velos logs <name> --since "1h"` / `--since "2026-02-12 10:00"`
- [ ] `velos logs <name> --until "2026-02-12 11:00"`
- [ ] `velos logs <name> --lines 100` — последние N строк
- [ ] Комбинирование фильтров: `--grep "timeout" --level error --since "1h"`

## 4.5 Deduplicator

- [ ] Нормализация сообщений (замена IP, UUID, чисел, timestamps на плейсхолдеры)
- [ ] Hash-based группировка нормализованных строк
- [ ] Sliding window (default: 60s)
- [ ] `velos logs <name> --dedupe` — вывод с дедупликацией
- [ ] Формат: `"<message> (x<count>, first: <time>, last: <time>)"`

## 4.6 Pattern Detector

- [ ] Frequency analysis: подсчёт повторений шаблонов за time window
- [ ] Trend detection: Rising / Stable / Declining
- [ ] Top-N patterns за период

## 4.7 Anomaly Detector

- [ ] Sliding window для error_rate и log_volume (по минутам)
- [ ] Вычисление mean + std_dev
- [ ] Anomaly: значение > mean + N*sigma (default: 2σ=warning, 3σ=critical)

## 4.8 Summary Generator

- [ ] `velos logs <name> --summary` — компактная сводка
- [ ] Вывод: total lines, by level, top patterns, anomalies, last error, health score
- [ ] Health score: эвристика 0-100 (на основе error rate, restarts, anomalies)

## 4.9 TUI Dashboard (`velos monit`)

- [ ] `velos-cli/src/tui/` — ratatui-based dashboard
- [ ] Таблица процессов с live-обновлением (CPU, RAM, status, restarts)
- [ ] CPU/RAM sparkline графики
- [ ] Live log viewer (переключение между процессами)
- [ ] Keyboard shortcuts: q=quit, ↑↓=select, r=restart, s=stop, l=logs, d=delete
- [ ] WebSocket stream от daemon для real-time данных (или periodic polling через IPC)

---

## Заметки

- Anomaly detector начинает работать только после накопления минимум 10 минут данных
- Health score: 100 - (errors * 5) - (anomalies * 10) - (restarts * 3), clamped to 0-100
- TUI обновляется каждые 2 секунды (configurable)
- Все алгоритмы работают на уровне кода — zero LLM costs
