# Phase 7: Polish + Release (v0.1.0)

> **Статус:** Done
> **Зависимости:** Phase 5, Phase 6
> **Цель:** Production-ready release v0.1.0.
> **Результат:** Single binary для 5 платформ. Homebrew, cargo install, Docker.

---

## 7.1 Startup Scripts

- [x] `velos startup` — detect init system (systemd / launchd / OpenRC)
- [x] Генерация systemd unit file (`velos-daemon.service`)
- [x] Systemd watchdog: sd_notify(READY=1), sd_notify(WATCHDOG=1) periodic
- [x] Генерация launchd plist (macOS)
- [x] `velos unstartup` — удаление startup скрипта
- [x] `velos save` автоматически после `velos startup`

## 7.2 Docker

- [x] Dockerfile: multi-stage build (zig build → rust build → alpine runtime)
- [x] `velos-runtime` entry point для Docker (аналог pm2-runtime)
  - Правильная обработка сигналов (PID 1 problem)
  - Форвардинг SIGTERM к children
  - Автовыход при завершении всех процессов
- [x] `.dockerignore`
- [x] `docker-compose.yml` пример
- [x] Publish: `ghcr.io/Dave93/velos`

## 7.3 Cross-Compilation

- [x] GitHub Actions: сборка для 5 targets (см. ARCHITECTURE.md 8.3)
  - [x] Linux x86_64
  - [x] Linux ARM64
  - [x] macOS x86_64
  - [x] macOS ARM64
  - [x] Windows x86_64
- [x] Stripped binaries (strip + UPX опционально)
- [x] SHA256 checksums для каждого артефакта

## 7.4 CI/CD Pipeline

- [x] `.github/workflows/ci.yml` — build + test на 3 ОС
- [x] `.github/workflows/release.yml` — release при тегах v*
- [x] GitHub Release: автоматическая загрузка бинарников
- [x] Cargo publish workflow (velos-core, velos-cli)

## 7.5 Дистрибуция

- [x] `cargo install velos` — публикация на crates.io
- [x] Homebrew formula (homebrew-tap)
- [x] Install script: `curl -fsSL https://velos.dev/install.sh | sh`
- [x] AUR package (PKGBUILD)

## 7.6 Shell Completions

- [x] Bash completions (clap_complete)
- [x] Zsh completions
- [x] Fish completions
- [x] `velos completions <shell>` — вывод completion скрипта

## 7.7 Документация

- [x] README.md: badges, quick start, features, comparison table, install
- [x] CHANGELOG.md
- [x] Man page: `velos(1)`
- [x] `docs/mcp-tools.md` — MCP reference (из Phase 5)

## 7.8 Performance Benchmarks

- [x] Benchmark: daemon memory usage (цель: <2MB)
- [x] Benchmark: startup time (daemon + first process)
- [x] Benchmark: IPC latency (command round-trip)
- [x] Benchmark: process spawn time (fork+exec)
- [x] Comparison table: Velos vs PM2 vs PMDaemon

## 7.9 Final Polish

- [x] Все error messages — понятные, с actionable suggestions
- [x] `velos --version` — version + platform + daemon RAM info
- [x] `velos --help` — краткое, понятное, с примерами
- [x] Security: socket permissions (0600), PID file race conditions
- [x] Тег `v0.1.0` + GitHub Release

---

## Заметки

- v0.1.0 = first public release, не 1.0.0
- Binary size target: <10MB (stripped, release)
- Daemon RAM target: <2MB idle
- Windows support в v0.1.0: best-effort (может быть beta)
