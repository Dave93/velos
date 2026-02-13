# Velos Performance Benchmarks

## Environment

- **Platform**: aarch64-apple-darwin (Apple Silicon)
- **Zig**: 0.15.2 (core daemon)
- **Rust**: stable (CLI + client)
- **Build**: debug (release will be faster)

## Results

| Metric | Velos | PM2 (approx.) | Target |
|--------|-------|----------------|--------|
| Binary size | ~5 MB | ~25 MB (Node.js) | <10 MB |
| Daemon RSS (idle) | ~1.5 MB | ~30 MB | <2 MB |
| Daemon RSS (11 procs) | ~2 MB | ~35 MB | <5 MB |
| Daemon startup | ~5 ms | ~500 ms | <50 ms |
| IPC ping latency | ~0.5 ms | ~5 ms | <1 ms |
| Process spawn (fork+exec) | ~5 ms | ~50 ms | <10 ms |
| List (11 procs) | ~1 ms | ~10 ms | <5 ms |

> **Note**: PM2 values are approximate estimates based on typical Node.js process manager overhead. Actual PM2 performance varies by system and version. Velos values are from the benchmark script (`benchmarks/bench.sh`).

## Why Velos is Fast

1. **Zig core**: The daemon is written in Zig with manual memory management — no GC pauses, no runtime overhead.
2. **Binary protocol**: IPC uses a compact binary protocol (7-byte header + MessagePack) instead of JSON-over-HTTP.
3. **Static binary**: Single executable, no interpreter or VM startup cost.
4. **Zero-copy where possible**: The daemon processes IPC messages without unnecessary allocations.
5. **kqueue/epoll event loop**: Native OS event notification, not libuv or tokio in the hot path.

## Running Benchmarks

```bash
# Build first
make dev   # or: cd zig && zig build && cd .. && cargo build

# Run benchmarks
bash benchmarks/bench.sh
```

The script starts an isolated daemon instance and cleans up automatically.
