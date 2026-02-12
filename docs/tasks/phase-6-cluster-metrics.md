# Phase 6: Clustering + Metrics + API

> **Статус:** Not started
> **Зависимости:** Phase 3
> **Цель:** Multi-instance management и production observability.
> **Результат:** Cluster mode, Prometheus /metrics, OpenTelemetry, REST API.

---

## 6.1 Cluster Mode

- [ ] `velos start app.js -i 4` — запуск N инстансов одного процесса
- [ ] `velos start app.js -i max` — число инстансов = число CPU cores
- [ ] Каждый инстанс получает `VELOS_INSTANCE_ID` env variable (0-based)
- [ ] `NODE_APP_INSTANCE` для совместимости с PM2
- [ ] `velos list` показывает каждый инстанс отдельно: `api:0`, `api:1`, `api:2`
- [ ] `velos stop api` — останавливает все инстансы
- [ ] `velos stop api:2` — останавливает конкретный инстанс

## 6.2 Scaling

- [ ] `velos scale <name> <count>` — установить точное число инстансов
- [ ] `velos scale <name> +2` — добавить инстансы
- [ ] `velos scale <name> -1` — убрать инстансы
- [ ] Rolling restart для zero-downtime: перезапуск по одному инстансу с паузой

## 6.3 Prometheus Metrics

- [ ] Создать `velos-metrics` crate
- [ ] `velos metrics --port 9615` — запуск HTTP сервера с /metrics endpoint
- [ ] Per-process метрики: cpu_percent, memory_bytes, uptime_seconds, restart_total, status
- [ ] Per-instance метрики (labels: name, instance)
- [ ] Daemon метрики: uptime, processes_total, memory_bytes, ipc_requests_total
- [ ] ipc_latency_seconds histogram

## 6.4 OpenTelemetry

- [ ] OTLP exporter (gRPC или HTTP)
- [ ] Traces: process lifecycle spans (start, restart, stop, error events)
- [ ] Resource attributes: service.name, service.version, host.name
- [ ] Configurable endpoint: `otel_endpoint` в TOML

## 6.5 REST API

- [ ] Создать `velos-api` crate (axum)
- [ ] `velos api --port 3100` — запуск HTTP API сервера
- [ ] `GET /api/processes` — список процессов
- [ ] `GET /api/processes/:name` — информация о процессе
- [ ] `POST /api/processes` — запуск нового процесса
- [ ] `DELETE /api/processes/:name` — остановка и удаление
- [ ] `POST /api/processes/:name/restart` — перезапуск
- [ ] `GET /api/logs/:name?lines=100&level=error` — логи с фильтрацией
- [ ] WebSocket: `ws://localhost:3100/ws` — real-time updates (процессы + метрики)
- [ ] Optional token-based auth (`api_token` в config)
- [ ] CORS middleware

---

## Заметки

- Prometheus и API серверы — опциональные, запускаются отдельными командами
- Cluster mode на этом этапе — только round-robin через отдельные процессы (не Node.js cluster module)
- Rolling restart: пауза между инстансами = kill_timeout + 2s (configurable)
- REST API и MCP Server используют один и тот же velos-client (не дублируют)
