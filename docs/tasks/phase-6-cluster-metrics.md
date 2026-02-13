# Phase 6: Clustering + Metrics + API

> **Статус:** Done
> **Зависимости:** Phase 3
> **Цель:** Multi-instance management и production observability.
> **Результат:** Cluster mode, Prometheus /metrics, OpenTelemetry, REST API.

---

## 6.1 Cluster Mode

- [x] `velos start app.js -i 4` — запуск N инстансов одного процесса
- [x] `velos start app.js -i max` — число инстансов = число CPU cores
- [x] Каждый инстанс получает `VELOS_INSTANCE_ID` env variable (0-based)
- [x] `NODE_APP_INSTANCE` для совместимости с PM2
- [x] `velos list` показывает каждый инстанс отдельно: `api:0`, `api:1`, `api:2`
- [x] `velos stop api` — останавливает все инстансы
- [x] `velos stop api:2` — останавливает конкретный инстанс

## 6.2 Scaling

- [x] `velos scale <name> <count>` — установить точное число инстансов
- [x] `velos scale <name> +2` — добавить инстансы
- [x] `velos scale <name> -1` — убрать инстансы
- [x] Rolling restart для zero-downtime: перезапуск по одному инстансу с паузой

## 6.3 Prometheus Metrics

- [x] Создать `velos-metrics` crate
- [x] `velos metrics --port 9615` — запуск HTTP сервера с /metrics endpoint
- [x] Per-process метрики: cpu_percent, memory_bytes, uptime_seconds, restart_total, status
- [x] Per-instance метрики (labels: name, instance)
- [x] Daemon метрики: uptime, processes_total, memory_bytes, ipc_requests_total
- [x] ipc_latency_seconds histogram

## 6.4 OpenTelemetry

- [x] OTLP exporter (gRPC или HTTP)
- [x] Traces: process lifecycle spans (start, restart, stop, error events)
- [x] Resource attributes: service.name, service.version, host.name
- [x] Configurable endpoint: `otel_endpoint` в TOML

## 6.5 REST API

- [x] Создать `velos-api` crate (axum)
- [x] `velos api --port 3100` — запуск HTTP API сервера
- [x] `GET /api/processes` — список процессов
- [x] `GET /api/processes/:name` — информация о процессе
- [x] `POST /api/processes` — запуск нового процесса
- [x] `DELETE /api/processes/:name` — остановка и удаление
- [x] `POST /api/processes/:name/restart` — перезапуск
- [x] `GET /api/logs/:name?lines=100&level=error` — логи с фильтрацией
- [x] WebSocket: `ws://localhost:3100/ws` — real-time updates (процессы + метрики)
- [x] Optional token-based auth (`api_token` в config)
- [x] CORS middleware

---

## Заметки

- Prometheus и API серверы — опциональные, запускаются отдельными командами
- Cluster mode на этом этапе — только round-robin через отдельные процессы (не Node.js cluster module)
- Rolling restart: пауза между инстансами = kill_timeout + 2s (configurable)
- REST API и MCP Server используют один и тот же velos-client (не дублируют)
