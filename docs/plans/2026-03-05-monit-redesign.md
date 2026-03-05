# Velos Monitor TUI Redesign

## Overview

Redesign `velos monit` from basic ratatui widgets to a modern, btop/lazydocker-inspired dashboard with Catppuccin Mocha color scheme, dual sparkline graphs, auto-streaming logs, and interactive features.

## Layout

```
+------------------------------------------------------------------+
| Velos Monitor | 5 procs | CPU: 12.3% | MEM: 1.2 GB | up 3h 42m | <- header bar
+------------------------------------------------------------------+
| ID | Name       | PID   | Status  | CPU%       | MEM    | Uptime | <- process table
| >1 | runner-api | 96737 | running | ####.. 42% | 278 MB | 3h 42m | <- selected row
|  2 | worker     | 96738 | running | ##.... 11% | 64 MB  | 3h 42m |
+------------------------------------------------------------------+
| CPU (runner-api)  4.2%   | Memory (runner-api)  278 MB            | <- dual sparklines
| ..##..####..##..##..#### | ########..########..########..######## |
+------------------------------------------------------------------+
| Logs (runner-api) auto                                            | <- auto-stream logs
| 09:55:32 [out] Elysia server running at localhost:3001            |
| 09:55:33 [err] WARN: Social provider apple missing clientSecret   |
+------------------------------------------------------------------+
| q quit  ^/v select  / filter  Enter detail  r restart  s stop    | <- footer
+------------------------------------------------------------------+
```

## Color Scheme: Catppuccin Mocha

| Element        | Color   | Hex     |
|----------------|---------|---------|
| Background     | Base    | #1e1e2e |
| Surface/Border | Surface0| #313244 |
| Text           | Text    | #cdd6f4 |
| Headers        | Lavender| #b4befe |
| Running        | Green   | #a6e3a1 |
| Errored        | Red     | #f38ba8 |
| Stopped        | Yellow  | #f9e2af |
| Starting       | Mauve   | #cba6f7 |
| CPU graph      | Peach   | #fab387 |
| MEM graph      | Blue    | #89b4fa |
| Selected row   | Surface1| #45475a |
| Footer keys    | Overlay0| #6c7086 |
| Footer values  | Subtext1| #a6adc8 |

## Tasks

### Phase 1: Visual foundation
- [ ] 1.1 Define Catppuccin Mocha color constants (module or const block)
- [ ] 1.2 Switch all Block borders to `BorderType::Rounded`
- [ ] 1.3 Redesign header bar — summary stats (process count, total CPU%, total MEM, daemon uptime)
- [ ] 1.4 Redesign footer — styled keybinding hints with color-coded keys

### Phase 2: Process table upgrade
- [ ] 2.1 Add CPU% column with inline gauge bar (colored bar + percentage text)
- [ ] 2.2 Color-coded status cells (green/red/yellow/mauve per status)
- [ ] 2.3 Selected row highlight with Surface1 background
- [ ] 2.4 Store `cpu_percent` in ProcessRow struct (read from ProcessInfo)

### Phase 3: Dual sparkline graphs
- [ ] 3.1 Split the graph area into two side-by-side panels (CPU left, MEM right)
- [ ] 3.2 CPU sparkline with Peach color, showing last 120 samples (4 min at 2s)
- [ ] 3.3 MEM sparkline with Blue color, same history window
- [ ] 3.4 Track CPU history per process (similar to existing mem_history)
- [ ] 3.5 Show current value in graph title (e.g., "CPU (runner-api) 4.2%")

### Phase 4: Auto-streaming logs
- [ ] 4.1 Fetch logs automatically every 2s for selected process (remove manual `l` requirement)
- [ ] 4.2 Color log lines: red for stderr, dim timestamp, bold error keywords
- [ ] 4.3 Show "auto" indicator in log panel title
- [ ] 4.4 Scroll support — keep latest logs visible, allow scrolling up with PageUp/PageDown

### Phase 5: Search and sort
- [ ] 5.1 `/` enters filter mode — type to filter processes by name (fuzzy or substring)
- [ ] 5.2 Show filter input in footer area, Esc to cancel
- [ ] 5.3 `Tab` cycles sort column: name -> cpu -> mem -> uptime -> restarts
- [ ] 5.4 Show sort indicator arrow in active column header

### Phase 6: Detail panel
- [ ] 6.1 `Enter` on selected process opens detail overlay/panel
- [ ] 6.2 Show: script, cwd, interpreter, config (autorestart, max_restarts, etc.)
- [ ] 6.3 Show: PID, status, uptime, CPU%, MEM, restart count, consecutive crashes
- [ ] 6.4 `Esc` or `Enter` closes detail panel

### Phase 7: Signal sending
- [ ] 7.1 `k` opens signal picker popup (SIGTERM, SIGKILL, SIGHUP, SIGUSR1, SIGUSR2)
- [ ] 7.2 Arrow keys to select signal, Enter to send
- [ ] 7.3 Esc to cancel

### Phase 8: Notifications and help
- [ ] 8.1 Toast notification on process crash/restart (bottom-right popup, auto-dismiss 5s)
- [ ] 8.2 `?` opens help popup with all keybindings listed
- [ ] 8.3 Esc closes help popup

## Dependencies

- `ratatui` 0.29+ (already in Cargo.toml)
- `crossterm` (already in Cargo.toml)
- No new crate dependencies needed — all achievable with built-in ratatui widgets

## Notes

- CPU% data comes from the existing `cpu_percent` field added in v0.1.7
- Total daemon uptime can be derived from the oldest process uptime or from daemon ping
- Inline CPU gauge bar is a custom-rendered cell (colored block chars + text)
- Catppuccin colors use RGB — ratatui `Color::Rgb(r, g, b)`
