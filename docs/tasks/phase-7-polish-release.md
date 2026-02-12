# Phase 7: Polish + Release (v0.1.0)

> **Статус:** Not started
> **Зависимости:** Phase 5, Phase 6
> **Цель:** Production-ready release v0.1.0.
> **Результат:** Single binary для 5 платформ. Homebrew, cargo install, Docker.

---

## 7.1 Startup Scripts

- [ ] `velos startup` — detect init system (systemd / launchd / OpenRC)
- [ ] Генерация systemd unit file (`velos-daemon.service`)
- [ ] Systemd watchdog: sd_notify(READY=1), sd_notify(WATCHDOG=1) periodic
- [ ] Генерация launchd plist (macOS)
- [ ] `velos unstartup` — удаление startup скрипта
- [ ] `velos save` автоматически после `velos startup`

## 7.2 Docker

- [ ] Dockerfile: multi-stage build (zig build → rust build → alpine runtime)
- [ ] `velos-runtime` entry point для Docker (аналог pm2-runtime)
  - Правильная обработка сигналов (PID 1 problem)
  - Форвардинг SIGTERM к children
  - Автовыход при завершении всех процессов
- [ ] `.dockerignore`
- [ ] `docker-compose.yml` пример
- [ ] Publish: `ghcr.io/user/velos`

## 7.3 Cross-Compilation

- [ ] GitHub Actions: сборка для 5 targets (см. ARCHITECTURE.md 8.3)
  - [ ] Linux x86_64
  - [ ] Linux ARM64
  - [ ] macOS x86_64
  - [ ] macOS ARM64
  - [ ] Windows x86_64
- [ ] Stripped binaries (strip + UPX опционально)
- [ ] SHA256 checksums для каждого артефакта

## 7.4 CI/CD Pipeline

- [ ] `.github/workflows/ci.yml` — build + test на 3 ОС
- [ ] `.github/workflows/release.yml` — release при тегах v*
- [ ] GitHub Release: автоматическая загрузка бинарников
- [ ] Cargo publish workflow (velos-core, velos-cli)

## 7.5 Дистрибуция

- [ ] `cargo install velos` — публикация на crates.io
- [ ] Homebrew formula (homebrew-tap)
- [ ] Install script: `curl -fsSL https://velos.dev/install.sh | sh`
- [ ] AUR package (PKGBUILD)

## 7.6 Shell Completions

- [ ] Bash completions (clap_complete)
- [ ] Zsh completions
- [ ] Fish completions
- [ ] `velos completions <shell>` — вывод completion скрипта

## 7.7 Документация

- [ ] README.md: badges, quick start, features, comparison table, install
- [ ] CHANGELOG.md
- [ ] Man page: `velos(1)`
- [ ] `docs/mcp-tools.md` — MCP reference (из Phase 5)

## 7.8 Performance Benchmarks

- [ ] Benchmark: daemon memory usage (цель: <2MB)
- [ ] Benchmark: startup time (daemon + first process)
- [ ] Benchmark: IPC latency (command round-trip)
- [ ] Benchmark: process spawn time (fork+exec)
- [ ] Comparison table: Velos vs PM2 vs PMDaemon

## 7.9 Final Polish

- [ ] Все error messages — понятные, с actionable suggestions
- [ ] `velos --version` — version + platform + daemon RAM info
- [ ] `velos --help` — краткое, понятное, с примерами
- [ ] Security: socket permissions (0600), PID file race conditions
- [ ] Тег `v0.1.0` + GitHub Release

---

## Заметки

- v0.1.0 = first public release, не 1.0.0
- Binary size target: <10MB (stripped, release)
- Daemon RAM target: <2MB idle
- Windows support в v0.1.0: best-effort (может быть beta)
