# Rust Binary Service

A compiled Rust binary managed by Velos with exponential backoff restarts and generous memory limits.

## What this configures

- **Process**: runs `./target/release/velos-example-service` directly (no interpreter)
- **Unlimited restarts** with exponential backoff (500ms, 1s, 2s, 4s, ...)
- **Memory guard**: restart if RSS exceeds 1 GB
- **RUST_BACKTRACE**: enabled for crash diagnostics
- **Env profiles**: `production` (warn-level logs) and `development` (debug + trace)

## Quick start

```bash
# Build the example binary
cargo build --release

# Update velos.toml to point to the binary
# script = "./target/release/velos-example-service"

# Start with Velos
velos start --config velos.toml --env production

# Test the server
curl http://localhost:8080/
curl http://localhost:8080/health

# Check status and resource usage
velos monit

# View logs with smart analysis
velos logs service --analyze
```

## Notes

- No `interpreter` field means Velos runs the binary directly via fork/exec
- `max_restarts = -1` means unlimited — use with `exp_backoff_restart_delay` to avoid tight restart loops
- `RUST_BACKTRACE=1` gives stack traces on panics, visible in `velos logs service`
- For graceful shutdown, use the `signal-hook` or `tokio::signal` crate in your real app
