# Contributing to Velos

Thank you for your interest in contributing to Velos. This guide will help you get started.

## Prerequisites

- [Zig](https://ziglang.org/) 0.15 or later
- [Rust](https://www.rust-lang.org/) 1.75 or later (with Cargo)
- macOS or Linux (Windows support is planned but not yet available)
- GNU Make

## Development Setup

1. Fork and clone the repository:

```bash
git clone https://github.com/<your-username>/velos.git
cd velos
```

2. Build the project in development mode:

```bash
make dev
```

This runs `zig build` (debug) followed by `cargo build` (debug).

3. Run the test suite:

```bash
make test
```

4. Run integration tests:

```bash
make dev && bash tests/integration_lifecycle.sh
```

## Architecture Overview

Velos is a hybrid Zig + Rust project:

- **Zig core** (`zig/src/`) -- The daemon, process management (fork/exec), IPC server (Unix socket), CPU/RAM monitoring via syscalls, log collection, and ring buffers. Compiled to a static library (`libvelos_core.a`).
- **Rust shell** (`crates/`) -- CLI (clap), IPC client, REST API (axum), MCP server, Smart Log Engine, Prometheus/OpenTelemetry exporter, config parser, and AI crash analysis.

The Zig core exports a C ABI that the Rust side links against via FFI (`velos-ffi` crate).

For full architectural details, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Code Style

- **Rust**: Use `rustfmt` with default settings. Run `cargo fmt --all` before committing.
- **Zig**: Use `zig fmt`. Run `zig fmt zig/src/` before committing.
- Keep functions focused and reasonably sized.
- Prefer explicit error handling over panics.

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
feat: add process grouping support
fix: resolve memory leak in log collector
chore: update dependencies
docs: clarify IPC protocol documentation
test: add integration tests for restart command
refactor: simplify daemon initialization
```

Use the imperative mood in the subject line. Keep the subject under 72 characters.

## Pull Request Process

1. **Fork** the repository and create a feature branch from `main`:

```bash
git checkout -b feat/my-feature
```

2. **Implement** your changes with appropriate tests.

3. **Test** thoroughly:

```bash
make test
bash tests/integration_lifecycle.sh
```

4. **Format** your code:

```bash
cargo fmt --all
zig fmt zig/src/
```

5. **Push** your branch and open a Pull Request against `main`.

6. Fill out the PR template and ensure all CI checks pass.

7. A maintainer will review your PR. Address any feedback promptly.

## Testing Requirements

All pull requests must:

- Pass existing unit tests (`cargo test`, `zig build test` in `zig/`)
- Pass integration tests (`tests/integration_lifecycle.sh`)
- Include new tests for new functionality
- Not break existing tests

If a feature cannot be tested in non-interactive mode (e.g., TUI), document it as `[MANUAL]` in the PR description.

## Where to Find Tasks

- Check [GitHub Issues](https://github.com/Dave93/velos/issues) for open tasks, bugs, and feature requests.
- Issues labeled `good first issue` are suitable for newcomers.
- If you plan to work on something significant, open an issue first to discuss the approach.

## Reporting Bugs

Use the [Bug Report](https://github.com/Dave93/velos/issues/new?template=bug_report.yml) issue template. Include your Velos version, OS, and steps to reproduce.

## Requesting Features

Use the [Feature Request](https://github.com/Dave93/velos/issues/new?template=feature_request.yml) issue template. Describe the problem and your proposed solution.

## License

By contributing to Velos, you agree that your contributions will be licensed under the same terms as the project: MIT OR Apache-2.0 (dual license).
