# Phase 4: Smart Logs + Monitoring

> **Статус:** Done (37/37)
> **Зависимости:** Phase 3
> **Цель:** Smart Log Engine и TUI мониторинг.
> **Результат:** `velos logs --summary`, `velos logs --dedupe`, `velos monit` (TUI).

---

## 4.1 Log Engine — инфраструктура

- [x] Создать `velos-log-engine` crate
- [x] Определить trait `LogProcessor` (pipeline stage interface)
- [x] Pipeline: raw stream → classifier → dedup → pattern → anomaly → output
- [x] Конфигурация pipeline через TOML (включение/отключение стадий)

## 4.2 Auto-Classifier

- [x] Regex-based level detection (error, warn, info, debug, fatal)
- [x] Default ruleset (см. ARCHITECTURE.md секция 5.2)
- [x] Custom rules из TOML конфигурации
- [x] JSON-aware: если строка уже JSON с полем "level" → использовать его

## 4.3 Structured Logs

- [x] Опциональный structured JSON Lines формат записи
- [x] Каждая строка: `{"ts":..., "lvl":..., "pid":..., "msg":..., "src":...}`
- [x] Fallback: plain text для совместимости (--raw)

## 4.4 Фильтрация логов (CLI)

- [x] `velos logs <name> --grep <pattern>` — regex фильтрация
- [x] `velos logs <name> --level error,warn` — фильтр по уровню
- [x] `velos logs <name> --since "1h"` / `--since "2026-02-12 10:00"`
- [x] `velos logs <name> --until "2026-02-12 11:00"`
- [x] `velos logs <name> --lines 100` — последние N строк
- [x] Комбинирование фильтров: `--grep "timeout" --level error --since "1h"`

## 4.5 Deduplicator

- [x] Нормализация сообщений (замена IP, UUID, чисел, timestamps на плейсхолдеры)
- [x] Hash-based группировка нормализованных строк
- [x] Sliding window (default: 60s)
- [x] `velos logs <name> --dedupe` — вывод с дедупликацией
- [x] Формат: `"<message> (x<count>, first: <time>, last: <time>)"`

## 4.6 Pattern Detector

- [x] Frequency analysis: подсчёт повторений шаблонов за time window
- [x] Trend detection: Rising / Stable / Declining
- [x] Top-N patterns за период

## 4.7 Anomaly Detector

- [x] Sliding window для error_rate и log_volume (по минутам)
- [x] Вычисление mean + std_dev
- [x] Anomaly: значение > mean + N*sigma (default: 2σ=warning, 3σ=critical)

## 4.8 Summary Generator

- [x] `velos logs <name> --summary` — компактная сводка
- [x] Вывод: total lines, by level, top patterns, anomalies, last error, health score
- [x] Health score: эвристика 0-100 (на основе error rate, restarts, anomalies)

## 4.9 TUI Dashboard (`velos monit`)

- [x] `velos-cli/src/commands/monit.rs` — ratatui-based dashboard
- [x] Таблица процессов с live-обновлением (CPU, RAM, status, restarts)
- [x] CPU/RAM sparkline графики
- [x] Live log viewer (переключение между процессами)
- [x] Keyboard shortcuts: q=quit, ↑↓=select, r=restart, s=stop, l=logs, d=delete
- [x] Periodic polling через IPC (каждые 2 секунды)

---

## Заметки

- Anomaly detector начинает работать только после накопления минимум 10 минут данных
- Health score: 100 - (errors * 5) - (anomalies * 10) - (restarts * 3), clamped to 0-100
- TUI обновляется каждые 2 секунды (configurable)
- Все алгоритмы работают на уровне кода — zero LLM costs
- Unit tests: 38 тестов в velos-log-engine (classifier: 10, format: 6, dedup: 5, pattern: 5, anomaly: 7, summary: 5)
