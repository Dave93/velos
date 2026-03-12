# Node.js Web Server

A typical Node.js HTTP server managed by Velos with file watching, memory limits, and environment profiles.

## What this configures

- **Process**: `node src/index.js` with a 512 MB V8 heap limit
- **Auto-restart**: on crash, up to 15 attempts with 1s delay
- **Memory guard**: restart if RSS exceeds 300 MB
- **File watching**: reloads on changes in `src/` and `config/`
- **Cron restart**: daily at 4 AM to reclaim memory
- **Env profiles**: `production` and `development`

## Quick start

```bash
# Start with Velos
velos start --config velos.toml

# Start in production
velos start --config velos.toml --env production

# Test the server
curl http://localhost:3000/
curl http://localhost:3000/health

# Check status
velos list

# View logs
velos logs web
```

## PM2 equivalent

```js
// ecosystem.config.js
module.exports = {
  apps: [{
    name: "web",
    script: "src/index.js",
    node_args: "--max-old-space-size=512",
    watch: ["src/", "config/"],
    max_memory_restart: "300M",
    env: { NODE_ENV: "production", PORT: "3000" },
    env_development: { NODE_ENV: "development" }
  }]
}
```

## Notes

- The server handles `SIGTERM` for graceful shutdown (Velos sends it before `kill_timeout`)
- `/health` endpoint returns uptime — useful for monitoring
- Set `watch = false` in production to avoid unnecessary filesystem overhead
