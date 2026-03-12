# Multi-Process Stack

A complete application stack — frontend, backend API, and background worker — all in one `velos.toml`.

## What this configures

| Process | Stack | Port | Memory Limit |
|---------|-------|------|-------------|
| `frontend` | Node.js | 3000 | 512 MB |
| `backend` | Node.js | 4000 | 300 MB |
| `worker` | Python | — | 256 MB |

- **Smart Log Engine**: enabled with dedup, pattern detection, and anomaly alerts
- **Env profiles**: separate database/Redis URLs for production and development
- **Auto-restart**: all processes restart on crash with individual limits

## Quick start

```bash
# Start the full stack
velos start --config velos.toml

# Start with production env
velos start --config velos.toml --env production

# Start individual processes
velos start frontend --config velos.toml
velos start backend --config velos.toml --env development

# Monitor all processes
velos monit

# View logs for a specific process
velos logs backend

# Analyze logs with smart engine
velos logs backend --analyze

# Stop everything
velos stop all
```

## Directory structure

```
multi-process/
├── velos.toml          # all 3 processes + log engine config
├── frontend/
│   └── server.js       # minimal HTTP server (:3000)
├── backend/
│   └── server.js       # minimal JSON API (:4000)
└── worker/
    └── tasks.py        # background task processor
```

## Notes

- Each process has its own `cwd`, so relative paths resolve correctly
- The `[logs]` section configures the Smart Log Engine globally for all processes
- Use `velos monit` to see real-time CPU/memory across all processes
- Replace the sample servers with your real application code
