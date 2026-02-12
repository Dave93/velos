# Phase 1: Skeleton + Build System

> **Статус:** Not started
> **Цель:** Работающий Zig+Rust гибрид. Zig-библиотека вызывается из Rust. Single binary.
> **Результат:** `velos ping` → "pong" (Rust CLI вызывает Zig через FFI)

---

## 1.1 Инициализация репозитория

- [ ] Инициализировать Git-репозиторий
- [ ] Создать .gitignore (zig-out, target, *.a, *.o, .velos/)
- [ ] Добавить LICENSE (MIT OR Apache-2.0)

## 1.2 Zig-ядро

- [ ] Создать `zig/build.zig` — build script для static library
- [ ] Создать `zig/build.zig.zon` — package manifest
- [ ] Создать `zig/src/lib.zig` — экспорт C ABI функций
  - [ ] `velos_ping()` → возвращает version string
  - [ ] `velos_daemon_init()` → заглушка, возвращает 0
- [ ] Создать `include/velos_core.h` — C header для экспортов
- [ ] Верифицировать: `cd zig && zig build` → `zig-out/lib/libvelos_core.a`

## 1.3 Rust workspace

- [ ] Создать корневой `Cargo.toml` (workspace)
- [ ] Создать `crates/velos-ffi/Cargo.toml`
- [ ] Создать `crates/velos-ffi/build.rs` — линковка libvelos_core.a
- [ ] Создать `crates/velos-ffi/src/lib.rs` — extern "C" declarations + safe wrappers
- [ ] Создать `crates/velos-core/Cargo.toml`
- [ ] Создать `crates/velos-core/src/lib.rs` — базовые типы (ProcessConfig, ProcessStatus, Error)
- [ ] Создать `crates/velos-cli/Cargo.toml`
- [ ] Создать `crates/velos-cli/src/main.rs` — clap setup, команда `velos ping`

## 1.4 Build system

- [ ] Создать Makefile с таргетами: build, build-debug, test, clean
- [ ] Верифицировать полный pipeline: `make build` → `target/release/velos`

## 1.5 Верификация

- [ ] `./target/release/velos ping` → выводит version + "pong from Zig core"
- [ ] `./target/release/velos --version` → "Velos 0.1.0-dev"
- [ ] `./target/release/velos --help` → список доступных команд
- [ ] Размер бинарника < 5MB (release, stripped)

---

## Заметки

- Zig build использует `-Doptimize=ReleaseFast` для release
- build.rs в velos-ffi должен вызывать `zig build` автоматически если libvelos_core.a отсутствует
- Все C ABI функции возвращают int (0 = ok, <0 = error code)
