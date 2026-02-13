const std = @import("std");
const posix = std.posix;

// Module imports
pub const pal_mod = @import("platform/pal.zig");
pub const macos_mod = @import("platform/macos.zig");
pub const protocol_mod = @import("ipc/protocol.zig");
pub const server_mod = @import("ipc/server.zig");
pub const supervisor_mod = @import("process/supervisor.zig");
pub const signals_mod = @import("process/signals.zig");
pub const collector_mod = @import("log/collector.zig");
pub const writer_mod = @import("log/writer.zig");
pub const ring_buffer_mod = @import("log/ring_buffer.zig");
pub const persistence_mod = @import("state/persistence.zig");
pub const watcher_mod = @import("watch/watcher.zig");
pub const cron_mod = @import("cron/parser.zig");
pub const ipc_channel_mod = @import("process/ipc_channel.zig");

// Re-export types for tests
const KqueueLoop = macos_mod.KqueueLoop;
const Watcher = watcher_mod.Watcher;
const CronExpr = cron_mod.CronExpr;
const IpcServer = server_mod.IpcServer;
const Supervisor = supervisor_mod.Supervisor;
const ProcessConfig = supervisor_mod.ProcessConfig;
const LogCollector = collector_mod.LogCollector;
const Persistence = persistence_mod.Persistence;

// ============================================================
// C ABI types
// ============================================================

pub const VelosProcessConfig = extern struct {
    name: [*:0]const u8,
    script: [*:0]const u8,
    cwd: [*:0]const u8,
    interpreter: ?[*:0]const u8, // NULL = auto
    kill_timeout_ms: u32, // default: 5000
    autorestart: bool,
    max_restarts: i32 = 15, // -1 = unlimited
    min_uptime_ms: u64 = 1000,
    restart_delay_ms: u32 = 0,
    exp_backoff: bool = false,
};

pub const VelosProcessInfo = extern struct {
    id: u32,
    name: [*:0]const u8,
    pid: u32,
    status: u8, // 0=stopped, 1=running, 2=errored, 3=starting
    memory_bytes: u64,
    uptime_ms: u64,
    restart_count: u32,
};

pub const VelosLogEntry = extern struct {
    timestamp_ms: u64,
    level: u8, // 0=debug, 1=info, 2=warn, 3=error
    stream: u8, // 0=stdout, 1=stderr
    message: [*]const u8,
    message_len: u32,
};

// ============================================================
// Global daemon state
// ============================================================

var g_allocator: std.mem.Allocator = undefined;
var g_event_loop: ?KqueueLoop = null;
var g_pal: ?pal_mod.Pal = null;
var g_ipc_server: ?IpcServer = null;
var g_supervisor: ?Supervisor = null;
var g_log_collector: ?LogCollector = null;
var g_persistence: ?Persistence = null;
var g_socket_path: ?[]u8 = null;
var g_running: bool = false;
var g_initialized: bool = false;

// ============================================================
// C ABI exports
// ============================================================

/// Returns a version/ping string. Exported via C ABI for FFI.
export fn velos_ping() [*:0]const u8 {
    return "Velos 0.1.0-dev - pong from Zig core";
}

/// Initialize the daemon. Sets up directories, event loop, IPC server, supervisor.
export fn velos_daemon_init(
    socket_path_c: ?[*:0]const u8,
    state_dir_c: ?[*:0]const u8,
) c_int {
    g_allocator = std.heap.c_allocator;

    // Determine state directory (must dupe — caller may free the C string)
    const state_dir = if (state_dir_c) |s|
        g_allocator.dupe(u8, std.mem.span(s)) catch return -1
    else blk: {
        // Default: ~/.velos
        const home = std.posix.getenv("HOME") orelse return -1;
        const dir = std.fmt.allocPrint(g_allocator, "{s}/.velos", .{home}) catch return -1;
        break :blk dir;
    };

    // Ensure directories exist
    g_persistence = Persistence.init(g_allocator, state_dir);
    g_persistence.?.ensureDirectories() catch return -2;

    // Write PID file
    g_persistence.?.writePidFile() catch return -3;

    // Get socket path
    const sock_path = if (socket_path_c) |s|
        g_allocator.dupe(u8, std.mem.span(s)) catch return -4
    else
        g_persistence.?.socketPath() catch return -4;

    g_socket_path = sock_path;

    // Get log directory (kept alive for daemon lifetime — no defer free)
    const log_dir = g_persistence.?.logDir() catch return -5;

    // Initialize log collector
    g_log_collector = LogCollector.init(g_allocator, log_dir);

    // Initialize event loop (kqueue on macOS)
    g_event_loop = KqueueLoop.init(g_allocator) catch return -6;
    g_pal = g_event_loop.?.asPal();

    // Initialize supervisor
    g_supervisor = Supervisor.init(g_allocator, &g_log_collector.?);

    // Initialize IPC server
    g_ipc_server = IpcServer.init(
        g_allocator,
        sock_path,
        &g_pal.?,
        &g_supervisor.?,
        &g_log_collector.?,
    ) catch return -7;

    // Wire persistence to IPC server for state save/load
    g_ipc_server.?.setPersistence(&g_persistence.?);

    // Register signals
    g_pal.?.addSignal(signals_mod.SIGCHLD) catch return -8;
    g_pal.?.addSignal(signals_mod.SIGTERM) catch return -8;
    g_pal.?.addSignal(signals_mod.SIGINT) catch return -8;

    g_initialized = true;
    return 0;
}

/// Run the main event loop (blocking). Returns 0 on clean shutdown.
export fn velos_daemon_run() c_int {
    if (!g_initialized) return -1;
    g_running = true;

    var events: [64]pal_mod.Event = undefined;

    while (g_running) {
        const n = g_pal.?.poll(&events, 1000) catch continue; // 1s timeout for pending kill checks

        for (events[0..n]) |event| {
            switch (event.kind) {
                .ipc_accept => {
                    g_ipc_server.?.acceptClient() catch {};
                },
                .ipc_read => {
                    g_ipc_server.?.handleClientData(event.fd) catch {};
                },
                .ipc_client_hup => {
                    g_ipc_server.?.removeClient(event.fd);
                },
                .pipe_read => {
                    g_log_collector.?.handlePipeData(event.fd) catch {};
                },
                .pipe_hup => {
                    g_pal.?.removeFd(event.fd);
                    g_log_collector.?.closePipe(event.fd);
                },
                .signal => {
                    if (event.signal_number == signals_mod.SIGCHLD) {
                        g_supervisor.?.handleSigchld() catch {};
                    } else if (event.signal_number == signals_mod.SIGTERM or event.signal_number == signals_mod.SIGINT) {
                        g_running = false;
                    }
                },
                .timer => {},
            }
        }

        // Check pending kills
        g_supervisor.?.checkPendingKills();

        // Check pending restarts (delayed/backoff autorestart)
        g_supervisor.?.checkPendingRestarts();

        // Register any new pipe fds from autorestart
        const pending_fds = g_supervisor.?.drainPendingPipeFds();
        if (pending_fds.len > 0) {
            for (pending_fds) |fd| {
                g_pal.?.addFd(fd, .pipe_read) catch {};
            }
            g_allocator.free(pending_fds);
        }

        // Periodic resource monitoring (every ~2s, checked inside)
        g_supervisor.?.updateResourceUsage();

        // Check watch mode — file changes trigger restart
        g_supervisor.?.checkWatchers();

        // Check cron-based restarts (once per minute)
        g_supervisor.?.checkCronRestarts();

        // Check wait_ready IPC channels
        g_supervisor.?.checkWaitReady();

        // Drain any new pipe fds from watch/cron restarts
        const watch_fds = g_supervisor.?.drainPendingPipeFds();
        if (watch_fds.len > 0) {
            for (watch_fds) |fd| {
                g_pal.?.addFd(fd, .pipe_read) catch {};
            }
            g_allocator.free(watch_fds);
        }

        // Check if shutdown was requested via IPC
        if (g_ipc_server.?.isShutdownRequested()) {
            g_running = false;
        }
    }

    // Graceful shutdown
    g_supervisor.?.stopAll();
    return 0;
}

/// Request daemon shutdown
export fn velos_daemon_shutdown() c_int {
    if (!g_initialized) return -1;
    g_running = false;

    // Cleanup
    if (g_ipc_server) |*srv| srv.deinit();
    if (g_supervisor) |*sup| sup.deinit();
    if (g_log_collector) |*lc| lc.deinit();
    if (g_event_loop) |*el| el.deinit();
    if (g_persistence) |*p| p.removePidFile();
    if (g_socket_path) |sp| g_allocator.free(sp);

    g_initialized = false;
    return 0;
}

/// Start a process. Returns process_id on success, negative on error.
export fn velos_process_start(config: ?*const VelosProcessConfig) c_int {
    if (!g_initialized) return -1;
    const cfg = config orelse return -2;

    const zig_config = ProcessConfig{
        .name = std.mem.span(cfg.name),
        .script = std.mem.span(cfg.script),
        .cwd = std.mem.span(cfg.cwd),
        .interpreter = if (cfg.interpreter) |i| std.mem.span(i) else null,
        .kill_timeout_ms = if (cfg.kill_timeout_ms == 0) 5000 else cfg.kill_timeout_ms,
        .autorestart = cfg.autorestart,
        .max_restarts = cfg.max_restarts,
        .min_uptime_ms = if (cfg.min_uptime_ms == 0) 1000 else cfg.min_uptime_ms,
        .restart_delay_ms = cfg.restart_delay_ms,
        .exp_backoff = cfg.exp_backoff,
    };

    const result = g_supervisor.?.startProcess(zig_config) catch return -3;

    // Register pipe fds in event loop
    g_pal.?.addFd(result.stdout_fd, .pipe_read) catch {};
    g_pal.?.addFd(result.stderr_fd, .pipe_read) catch {};

    return @intCast(result.id);
}

/// Stop a process. Returns 0 on success.
export fn velos_process_stop(process_id: u32, signal: c_int, timeout_ms: u32) c_int {
    if (!g_initialized) return -1;

    const sig: u8 = if (signal == 0) signals_mod.SIGTERM else @intCast(signal);
    const timeout = if (timeout_ms == 0) @as(u32, 5000) else timeout_ms;

    g_supervisor.?.stopProcess(process_id, sig, timeout) catch return -2;
    return 0;
}

/// Restart a process. Returns 0 on success.
export fn velos_process_restart(process_id: u32) c_int {
    if (!g_initialized) return -1;

    const result = g_supervisor.?.restartProcess(process_id) catch return -2;

    // Register new pipe fds in event loop
    g_pal.?.addFd(result.stdout_fd, .pipe_read) catch {};
    g_pal.?.addFd(result.stderr_fd, .pipe_read) catch {};

    // Also drain any pending pipe fds
    const pending_fds = g_supervisor.?.drainPendingPipeFds();
    if (pending_fds.len > 0) {
        for (pending_fds) |fd| {
            g_pal.?.addFd(fd, .pipe_read) catch {};
        }
        g_allocator.free(pending_fds);
    }

    return 0;
}

/// Delete a process. Returns 0 on success.
export fn velos_process_delete(process_id: u32) c_int {
    if (!g_initialized) return -1;
    g_supervisor.?.deleteProcess(process_id) catch return -2;
    return 0;
}

/// List all processes. Caller must call velos_process_list_free() on the result.
export fn velos_process_list(out: ?*?[*]VelosProcessInfo, count: ?*u32) c_int {
    if (!g_initialized) return -1;
    const out_ptr = out orelse return -2;
    const count_ptr = count orelse return -2;

    const procs = g_supervisor.?.listProcesses() catch return -3;
    defer g_supervisor.?.freeProcessList(procs);

    if (procs.len == 0) {
        out_ptr.* = null;
        count_ptr.* = 0;
        return 0;
    }

    // Allocate C-compatible array
    const c_procs = g_allocator.alloc(VelosProcessInfo, procs.len) catch return -4;

    for (procs, 0..) |proc, i| {
        // Allocate null-terminated name string
        const name_z = g_allocator.dupeZ(u8, proc.name) catch {
            // Free already allocated on error
            for (c_procs[0..i]) |*prev| {
                g_allocator.free(std.mem.span(prev.name));
            }
            g_allocator.free(c_procs);
            return -4;
        };

        c_procs[i] = VelosProcessInfo{
            .id = proc.id,
            .name = name_z.ptr,
            .pid = @intCast(proc.pid),
            .status = @intFromEnum(proc.status),
            .memory_bytes = proc.memory_bytes,
            .uptime_ms = proc.uptime_ms,
            .restart_count = proc.restart_count,
        };
    }

    out_ptr.* = c_procs.ptr;
    count_ptr.* = @intCast(procs.len);
    return 0;
}

/// Free a process list returned by velos_process_list
export fn velos_process_list_free(list: ?[*]VelosProcessInfo, count: u32) void {
    if (list) |ptr| {
        const slice = ptr[0..count];
        for (slice) |*info| {
            g_allocator.free(std.mem.span(info.name));
        }
        g_allocator.free(slice);
    }
}

/// Read log entries for a process. Caller must call velos_log_free().
export fn velos_log_read(
    process_id: u32,
    lines: u32,
    out: ?*?[*]VelosLogEntry,
    count: ?*u32,
) c_int {
    if (!g_initialized) return -1;
    const out_ptr = out orelse return -2;
    const count_ptr = count orelse return -2;

    const entries = g_log_collector.?.readLast(process_id, lines) catch return -3;
    defer g_log_collector.?.freeEntries(entries);

    if (entries.len == 0) {
        out_ptr.* = null;
        count_ptr.* = 0;
        return 0;
    }

    const c_entries = g_allocator.alloc(VelosLogEntry, entries.len) catch return -4;

    for (entries, 0..) |entry, i| {
        const msg_copy = g_allocator.dupe(u8, entry.message) catch {
            // Free already allocated
            for (c_entries[0..i]) |*prev| {
                g_allocator.free(prev.message[0..prev.message_len]);
            }
            g_allocator.free(c_entries);
            return -4;
        };

        c_entries[i] = VelosLogEntry{
            .timestamp_ms = entry.timestamp_ms,
            .level = entry.level,
            .stream = entry.stream,
            .message = msg_copy.ptr,
            .message_len = @intCast(msg_copy.len),
        };
    }

    out_ptr.* = c_entries.ptr;
    count_ptr.* = @intCast(entries.len);
    return 0;
}

/// Free log entries returned by velos_log_read
export fn velos_log_free(entries: ?[*]VelosLogEntry, count: u32) void {
    if (entries) |ptr| {
        const slice = ptr[0..count];
        for (slice) |*entry| {
            g_allocator.free(entry.message[0..entry.message_len]);
        }
        g_allocator.free(slice);
    }
}

/// Save all process configs to state file. Returns 0 on success.
export fn velos_state_save() c_int {
    if (!g_initialized) return -1;
    const p = &g_persistence.?;

    const procs = g_supervisor.?.getAllConfigs() catch return -2;
    defer g_allocator.free(procs);

    p.saveState(procs) catch return -3;
    return 0;
}

/// Load process configs from state file and start them. Returns number of processes started, negative on error.
export fn velos_state_load() c_int {
    if (!g_initialized) return -1;
    const p = &g_persistence.?;

    const configs = p.loadState() catch return -2;
    defer {
        for (configs) |cfg| {
            g_allocator.free(cfg.name);
            g_allocator.free(cfg.script);
            g_allocator.free(cfg.cwd);
            if (cfg.interpreter) |interp| g_allocator.free(interp);
        }
        g_allocator.free(configs);
    }

    var started: c_int = 0;
    for (configs) |cfg| {
        const result = g_supervisor.?.startProcess(cfg) catch continue;
        g_pal.?.addFd(result.stdout_fd, .pipe_read) catch {};
        g_pal.?.addFd(result.stderr_fd, .pipe_read) catch {};
        started += 1;
    }

    return started;
}

// ============================================================
// Tests - pull in all module tests
// ============================================================

test "velos_ping returns expected string" {
    const result = std.mem.span(velos_ping());
    try std.testing.expectEqualStrings("Velos 0.1.0-dev - pong from Zig core", result);
}

comptime {
    // Force test runner to include tests from sub-modules
    _ = ring_buffer_mod;
    _ = protocol_mod;
    _ = watcher_mod;
    _ = cron_mod;
    _ = ipc_channel_mod;
}
