# Python FastAPI

A FastAPI application running on Uvicorn, managed by Velos.

## What this configures

- **API**: `uvicorn main:app` with 4 workers, 512 MB memory limit
- **Auto-restart**: restarts on crash, up to 10 attempts
- **Log rotation**: 20 MB per file, 14 rotated files retained
- **Env profiles**: `production` and `development`

## Quick start

```bash
# Install dependencies
pip install -r requirements.txt

# Start with Velos
velos start --config velos.toml

# Start in development (with file watching)
# Set watch = true in velos.toml first
velos start --config velos.toml --env development

# Test the API
curl http://localhost:8000/
curl http://localhost:8000/health

# View logs
velos logs api
```

## Adding Celery

The config includes a commented-out Celery worker section. To enable it:

1. Install Celery: `pip install celery[redis]`
2. Uncomment the `[apps.celery]` section in `velos.toml`
3. Start a Redis broker: `redis-server`
4. Run: `velos start --config velos.toml`

## Notes

- `PYTHONUNBUFFERED=1` ensures Python output appears in logs immediately
- Adjust `--workers` based on your CPU cores (rule of thumb: `2 * cores + 1`)
- For development, set `watch = true` to auto-reload on code changes
