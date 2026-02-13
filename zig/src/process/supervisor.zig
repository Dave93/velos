const std = @import("std");
const builtin = @import("builtin");
const posix = std.posix;
const signals = @import("signals.zig");
const LogCollector = @import("../log/collector.zig").LogCollector;
const Watcher = @import("../watch/watcher.zig").Watcher;
const CronExpr = @import("../cron/parser.zig").CronExpr;
const IpcChannel = @import("ipc_channel.zig").IpcChannel;

pub const ProcessStatus = enum(u8) {
    stopped = 0,
    running = 1,
    errored = 2,
    starting = 3,
};

pub const ProcessConfig = struct {
    name: []const u8,
    script: []const u8,
    cwd: []const u8,
    interpreter: ?[]const u8, // null = auto-detect
    kill_timeout_ms: u32 = 5000,
    autorestart: bool = false,
    max_restarts: i32 = 15, // -1 = unlimited
    min_uptime_ms: u64 = 1000,
    restart_delay_ms: u32 = 0,
    exp_backoff: bool = false,
    log_max_size: u64 = 10 * 1024 * 1024, // 10MB
    log_retain_count: u32 = 30,
    max_memory_restart: u64 = 0, // 0 = unlimited, bytes
    watch: bool = false,
    watch_delay_ms: u32 = 1000,
    watch_paths: ?[]const u8 = null, // semicolon-separated
    watch_ignore: ?[]const u8 = null, // semicolon-separated
    cron_restart: ?[]const u8 = null,
    wait_ready: bool = false,
    listen_timeout_ms: u32 = 8000,
    shutdown_with_message: bool = false,
    instances: u32 = 1, // cluster mode: number of instances
    instance_id: u32 = 0, // this instance's 0-based ID
};

pub const ProcessInfo = struct {
    id: u32,
    name: []const u8, // owned
    pid: posix.pid_t,
    status: ProcessStatus,
    memory_bytes: u64,
    uptime_ms: u64,
    restart_count: u32,
    start_time_ms: u64,
    config: ProcessConfig, // stored config (strings owned)
    consecutive_crashes: u32 = 0,
    last_restart_ms: u64 = 0,
    instance_id: u32 = 0, // cluster instance ID
};

pub const Supervisor = struct {
    const Self = @This();

    processes: std.AutoHashMap(u32, *ProcessInfo),
    pid_to_id: std.AutoHashMap(posix.pid_t, u32),
    next_id: u32,
    log_collector: *LogCollector,
    allocator: std.mem.Allocator,

    // Pending kills (process_id -> deadline_ms for SIGKILL)
    pending_kills: std.AutoHashMap(u32, u64),

    // Pending restarts (process_id -> restart_at_ms) for delayed/backoff restarts
    pending_restarts: std.AutoHashMap(u32, u64),

    // Pipe fds from restarts that need to be registered in event loop
    pending_pipe_fds: std.ArrayList(posix.fd_t),

    // Last resource monitoring timestamp
    last_resource_check_ms: u64 = 0,

    // Watch mode: per-process file watchers (process_id → *Watcher)
    watchers: std.AutoHashMap(u32, *Watcher),

    // Cron restart: parsed cron expressions (process_id → CronExpr)
    cron_exprs: std.AutoHashMap(u32, CronExpr),

    // IPC channels for wait_ready/shutdown_with_message (process_id → IpcChannel)
    ipc_channels: std.AutoHashMap(u32, IpcChannel),

    // Track last cron check minute to avoid re-triggering
    last_cron_minute: i32 = -1,

    pub fn init(allocator: std.mem.Allocator, log_collector: *LogCollector) Self {
        return Self{
            .processes = std.AutoHashMap(u32, *ProcessInfo).init(allocator),
            .pid_to_id = std.AutoHashMap(posix.pid_t, u32).init(allocator),
            .next_id = 1,
            .log_collector = log_collector,
            .allocator = allocator,
            .pending_kills = std.AutoHashMap(u32, u64).init(allocator),
            .pending_restarts = std.AutoHashMap(u32, u64).init(allocator),
            .pending_pipe_fds = .{},
            .last_resource_check_ms = 0,
            .watchers = std.AutoHashMap(u32, *Watcher).init(allocator),
            .cron_exprs = std.AutoHashMap(u32, CronExpr).init(allocator),
            .ipc_channels = std.AutoHashMap(u32, IpcChannel).init(allocator),
            .last_cron_minute = -1,
        };
    }

    pub fn deinit(self: *Self) void {
        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            const proc = proc_ptr.*;
            self.allocator.free(proc.name);
            self.allocator.free(proc.config.name);
            self.allocator.free(proc.config.script);
            self.allocator.free(proc.config.cwd);
            if (proc.config.interpreter) |interp| self.allocator.free(interp);
            if (proc.config.watch_paths) |p| self.allocator.free(p);
            if (proc.config.watch_ignore) |p| self.allocator.free(p);
            if (proc.config.cron_restart) |c| self.allocator.free(c);
            self.allocator.destroy(proc);
        }
        self.processes.deinit();
        self.pid_to_id.deinit();
        self.pending_kills.deinit();
        self.pending_restarts.deinit();
        self.pending_pipe_fds.deinit(self.allocator);

        // Cleanup watchers
        var wit = self.watchers.valueIterator();
        while (wit.next()) |w_ptr| {
            var w = w_ptr.*;
            w.deinit();
            self.allocator.destroy(w);
        }
        self.watchers.deinit();

        // Cleanup cron expressions (no dynamic memory)
        self.cron_exprs.deinit();

        // Cleanup IPC channels
        var cit = self.ipc_channels.valueIterator();
        while (cit.next()) |ch_ptr| {
            var ch = ch_ptr.*;
            ch.deinit();
        }
        self.ipc_channels.deinit();
    }

    /// Start a new process. Returns process_id and the pipe fds to register in kqueue.
    pub fn startProcess(self: *Self, config: ProcessConfig) !struct { id: u32, stdout_fd: posix.fd_t, stderr_fd: posix.fd_t } {
        const id = self.next_id;
        self.next_id += 1;

        // Create pipes for stdout and stderr
        const stdout_pipe = try posix.pipe();
        const stderr_pipe = try posix.pipe();

        // Build argv
        var argv_list: std.ArrayList(?[*:0]const u8) = .{};
        defer argv_list.deinit(self.allocator);

        if (config.interpreter) |interp| {
            const interp_z = try self.allocator.dupeZ(u8, interp);
            defer self.allocator.free(interp_z);
            try argv_list.append(self.allocator, interp_z);
        } else {
            // Auto-detect: check shebang first, then extension
            const detected = self.detectInterpreter(config.script, config.cwd);
            if (detected) |interp| {
                try argv_list.append(self.allocator, interp);
                // For .ts/.tsx, npx needs "tsx" as next arg
                if (needsTsxArg(config.script)) {
                    try argv_list.append(self.allocator, "tsx");
                }
            }
        }

        const script_z = try self.allocator.dupeZ(u8, config.script);
        defer self.allocator.free(script_z);
        try argv_list.append(self.allocator, script_z);
        try argv_list.append(self.allocator, null); // null terminator

        const cwd_z = try self.allocator.dupeZ(u8, config.cwd);
        defer self.allocator.free(cwd_z);

        // Create IPC channel for wait_ready/shutdown_with_message
        var ipc_chan: ?IpcChannel = null;
        if (config.wait_ready or config.shutdown_with_message) {
            ipc_chan = IpcChannel.create() catch null;
        }

        const pid = try posix.fork();

        if (pid == 0) {
            // Child process
            // Close read ends of pipes
            posix.close(stdout_pipe[0]);
            posix.close(stderr_pipe[0]);

            // Redirect stdout and stderr to pipe write ends
            posix.dup2(stdout_pipe[1], posix.STDOUT_FILENO) catch posix.exit(127);
            posix.dup2(stderr_pipe[1], posix.STDERR_FILENO) catch posix.exit(127);

            // Close the now-duplicated write ends
            posix.close(stdout_pipe[1]);
            posix.close(stderr_pipe[1]);

            // Set up IPC channel env var for wait_ready/shutdown_with_message
            if (ipc_chan) |ch| {
                posix.close(ch.parent_fd); // close parent end in child
                setIpcEnv(ch.child_fd);
            }

            // Set cluster instance env vars
            if (config.instances > 1) {
                setInstanceEnv(config.instance_id);
            }

            // Change working directory
            _ = std.c.chdir(cwd_z);

            // Create a new session
            _ = std.c.setsid();

            // Exec
            const argv = argv_list.items;
            _ = posix.execvpeZ(argv[0].?, @ptrCast(argv.ptr), std.c.environ) catch {};
            posix.exit(127);
        }

        // Parent process
        // Close write ends of pipes (child owns them)
        posix.close(stdout_pipe[1]);
        posix.close(stderr_pipe[1]);

        // Set read ends to non-blocking
        setNonBlocking(stdout_pipe[0]);
        setNonBlocking(stderr_pipe[0]);

        // Store process info
        const proc = try self.allocator.create(ProcessInfo);
        proc.* = ProcessInfo{
            .id = id,
            .name = try self.allocator.dupe(u8, config.name),
            .pid = pid,
            .status = .running,
            .memory_bytes = 0,
            .uptime_ms = 0,
            .restart_count = 0,
            .start_time_ms = @intCast(std.time.milliTimestamp()),
            .config = ProcessConfig{
                .name = try self.allocator.dupe(u8, config.name),
                .script = try self.allocator.dupe(u8, config.script),
                .cwd = try self.allocator.dupe(u8, config.cwd),
                .interpreter = if (config.interpreter) |i| try self.allocator.dupe(u8, i) else null,
                .kill_timeout_ms = config.kill_timeout_ms,
                .autorestart = config.autorestart,
                .max_restarts = config.max_restarts,
                .min_uptime_ms = config.min_uptime_ms,
                .restart_delay_ms = config.restart_delay_ms,
                .exp_backoff = config.exp_backoff,
                .log_max_size = config.log_max_size,
                .log_retain_count = config.log_retain_count,
                .max_memory_restart = config.max_memory_restart,
                .watch = config.watch,
                .watch_delay_ms = config.watch_delay_ms,
                .watch_paths = if (config.watch_paths) |p| try self.allocator.dupe(u8, p) else null,
                .watch_ignore = if (config.watch_ignore) |p| try self.allocator.dupe(u8, p) else null,
                .cron_restart = if (config.cron_restart) |c| try self.allocator.dupe(u8, c) else null,
                .wait_ready = config.wait_ready,
                .listen_timeout_ms = config.listen_timeout_ms,
                .shutdown_with_message = config.shutdown_with_message,
                .instances = config.instances,
                .instance_id = config.instance_id,
            },
            .instance_id = config.instance_id,
        };

        try self.processes.put(id, proc);
        try self.pid_to_id.put(pid, id);

        // Register with log collector
        try self.log_collector.addProcess(id, config.name, stdout_pipe[0], stderr_pipe[0]);
        self.log_collector.setLogConfig(id, config.log_max_size, config.log_retain_count);

        // Store IPC channel and configure process status
        if (ipc_chan) |ch| {
            var stored_ch = ch;
            stored_ch.closeChildEnd();
            self.ipc_channels.put(id, stored_ch) catch {};
            if (config.wait_ready) {
                proc.status = .starting;
            }
        }

        // Setup watcher and cron for this process
        self.setupWatchAndCron(id);

        return .{
            .id = id,
            .stdout_fd = stdout_pipe[0],
            .stderr_fd = stderr_pipe[0],
        };
    }

    /// Initiate graceful stop: send SIGTERM and set kill deadline.
    pub fn stopProcess(self: *Self, process_id: u32, sig: u8, timeout_ms: u32) !void {
        const proc = self.processes.get(process_id) orelse return error.ProcessNotFound;
        if (proc.status != .running and proc.status != .starting) return;

        // Send shutdown message via IPC if configured
        if (proc.config.shutdown_with_message) {
            if (self.ipc_channels.getPtr(process_id)) |ch| {
                IpcChannel.sendMessage(ch.parent_fd, "{\"type\":\"shutdown\"}") catch {};
            }
        }

        // Send signal
        signals.sendSignal(proc.pid, sig) catch {};

        if (sig != signals.SIGKILL) {
            // Set SIGKILL deadline
            const deadline = @as(u64, @intCast(std.time.milliTimestamp())) + timeout_ms;
            try self.pending_kills.put(process_id, deadline);
        }

        proc.status = .stopped;
    }

    /// Delete a process entry (must be stopped first)
    pub fn deleteProcess(self: *Self, process_id: u32) !void {
        const proc = self.processes.get(process_id) orelse return error.ProcessNotFound;

        if (proc.status == .running) {
            // Force kill
            signals.sendSignal(proc.pid, signals.SIGKILL) catch {};
        }

        // Cleanup
        _ = self.pid_to_id.remove(proc.pid);
        self.log_collector.removeProcess(process_id);
        _ = self.pending_kills.remove(process_id);
        _ = self.pending_restarts.remove(process_id);

        // Cleanup watcher
        if (self.watchers.get(process_id)) |w| {
            var wm = w;
            wm.deinit();
            self.allocator.destroy(w);
        }
        _ = self.watchers.remove(process_id);

        // Cleanup cron
        _ = self.cron_exprs.remove(process_id);

        // Cleanup IPC channel
        if (self.ipc_channels.getPtr(process_id)) |ch| {
            ch.deinit();
        }
        _ = self.ipc_channels.remove(process_id);

        self.allocator.free(proc.name);
        self.allocator.free(proc.config.name);
        self.allocator.free(proc.config.script);
        self.allocator.free(proc.config.cwd);
        if (proc.config.interpreter) |interp| self.allocator.free(interp);
        if (proc.config.watch_paths) |p| self.allocator.free(p);
        if (proc.config.watch_ignore) |p| self.allocator.free(p);
        if (proc.config.cron_restart) |c| self.allocator.free(c);
        self.allocator.destroy(proc);
        _ = self.processes.remove(process_id);
    }

    /// Called when SIGCHLD is received. Reaps children and updates status.
    /// Implements autorestart with crash loop detection and exponential backoff.
    pub fn handleSigchld(self: *Self) !void {
        const reaped = try signals.reapChildren(self.allocator);
        defer self.allocator.free(reaped);

        const now: u64 = @intCast(std.time.milliTimestamp());

        for (reaped) |reap| {
            const process_id = self.pid_to_id.get(reap.pid) orelse continue;
            const proc = self.processes.get(process_id) orelse continue;

            _ = self.pending_kills.remove(process_id);
            _ = self.pid_to_id.remove(reap.pid);

            const was_running = proc.status == .running;
            const abnormal_exit = reap.signaled or reap.exit_code != 0;

            if (abnormal_exit) {
                proc.status = .errored;
            } else {
                proc.status = .stopped;
            }

            // Autorestart logic
            if (!was_running or !proc.config.autorestart) continue;

            // Calculate uptime of the process that just died
            const uptime = now -| proc.start_time_ms;

            // min_uptime check: if lived less than min_uptime_ms, it's a crash
            if (uptime < proc.config.min_uptime_ms) {
                proc.consecutive_crashes += 1;
            } else {
                proc.consecutive_crashes = 0;
            }

            // Crash loop detection: if max_restarts reached, stop restarting
            if (proc.config.max_restarts >= 0) {
                if (proc.consecutive_crashes >= @as(u32, @intCast(proc.config.max_restarts))) {
                    proc.status = .errored;
                    continue;
                }
            }

            // Calculate restart delay
            var delay: u64 = proc.config.restart_delay_ms;
            if (proc.config.exp_backoff and proc.consecutive_crashes > 0) {
                // delay = restart_delay_ms * 2^(consecutive_crashes - 1), capped at 15s
                const base = if (delay == 0) @as(u64, 100) else delay; // minimum 100ms for backoff
                const exp = @min(proc.consecutive_crashes - 1, 20); // cap exponent
                const shift: u6 = @intCast(exp);
                delay = @min(base << shift, 15000);
            }

            if (delay > 0) {
                // Schedule delayed restart
                self.pending_restarts.put(process_id, now + delay) catch {};
            } else {
                // Restart immediately
                self.doRestart(process_id, proc) catch {
                    proc.status = .errored;
                };
            }
        }
    }

    /// Execute a restart for a process: create new pipes, fork/exec, update state.
    fn doRestart(self: *Self, process_id: u32, proc: *ProcessInfo) !void {
        // Close old log pipes for this process
        self.log_collector.removeProcess(process_id);

        // Create new pipes
        const stdout_pipe = try posix.pipe();
        const stderr_pipe = try posix.pipe();

        // Build argv
        var argv_list: std.ArrayList(?[*:0]const u8) = .{};
        defer argv_list.deinit(self.allocator);

        if (proc.config.interpreter) |interp| {
            const interp_z = try self.allocator.dupeZ(u8, interp);
            defer self.allocator.free(interp_z);
            try argv_list.append(self.allocator, interp_z);
        } else {
            const detected = self.detectInterpreter(proc.config.script, proc.config.cwd);
            if (detected) |interp| {
                try argv_list.append(self.allocator, interp);
                if (needsTsxArg(proc.config.script)) {
                    try argv_list.append(self.allocator, "tsx");
                }
            }
        }

        const script_z = try self.allocator.dupeZ(u8, proc.config.script);
        defer self.allocator.free(script_z);
        try argv_list.append(self.allocator, script_z);
        try argv_list.append(self.allocator, null);

        const cwd_z = try self.allocator.dupeZ(u8, proc.config.cwd);
        defer self.allocator.free(cwd_z);

        // Create IPC channel for wait_ready/shutdown_with_message
        var ipc_chan: ?IpcChannel = null;
        if (proc.config.wait_ready or proc.config.shutdown_with_message) {
            ipc_chan = IpcChannel.create() catch null;
        }

        // Close old IPC channel if exists
        if (self.ipc_channels.getPtr(process_id)) |old_ch| {
            old_ch.deinit();
        }
        _ = self.ipc_channels.remove(process_id);

        const pid = try posix.fork();

        if (pid == 0) {
            // Child
            posix.close(stdout_pipe[0]);
            posix.close(stderr_pipe[0]);
            posix.dup2(stdout_pipe[1], posix.STDOUT_FILENO) catch posix.exit(127);
            posix.dup2(stderr_pipe[1], posix.STDERR_FILENO) catch posix.exit(127);
            posix.close(stdout_pipe[1]);
            posix.close(stderr_pipe[1]);

            // Set up IPC channel env var
            if (ipc_chan) |ch| {
                posix.close(ch.parent_fd);
                setIpcEnv(ch.child_fd);
            }

            // Set cluster instance env vars on restart
            if (proc.config.instances > 1) {
                setInstanceEnv(proc.config.instance_id);
            }

            _ = std.c.chdir(cwd_z);
            _ = std.c.setsid();
            const argv = argv_list.items;
            _ = posix.execvpeZ(argv[0].?, @ptrCast(argv.ptr), std.c.environ) catch {};
            posix.exit(127);
        }

        // Parent
        posix.close(stdout_pipe[1]);
        posix.close(stderr_pipe[1]);
        setNonBlocking(stdout_pipe[0]);
        setNonBlocking(stderr_pipe[0]);

        const now: u64 = @intCast(std.time.milliTimestamp());
        proc.pid = pid;
        proc.status = .running;
        proc.restart_count += 1;
        proc.start_time_ms = now;
        proc.last_restart_ms = now;

        // Store IPC channel
        if (ipc_chan) |ch| {
            var stored_ch = ch;
            stored_ch.closeChildEnd();
            self.ipc_channels.put(process_id, stored_ch) catch {};
            if (proc.config.wait_ready) {
                proc.status = .starting;
            }
        }

        try self.pid_to_id.put(pid, process_id);
        try self.log_collector.addProcess(process_id, proc.config.name, stdout_pipe[0], stderr_pipe[0]);

        // Return pipe fds for event loop registration (caller must handle)
        // We store them as pending_pipe_fds for the event loop to pick up
        try self.pending_pipe_fds.append(self.allocator, stdout_pipe[0]);
        try self.pending_pipe_fds.append(self.allocator, stderr_pipe[0]);
    }

    /// Check and execute any pending delayed restarts
    pub fn checkPendingRestarts(self: *Self) void {
        const now: u64 = @intCast(std.time.milliTimestamp());

        var to_restart: std.ArrayList(u32) = .{};
        defer to_restart.deinit(self.allocator);

        var it = self.pending_restarts.iterator();
        while (it.next()) |entry| {
            if (now >= entry.value_ptr.*) {
                to_restart.append(self.allocator, entry.key_ptr.*) catch continue;
            }
        }

        for (to_restart.items) |process_id| {
            _ = self.pending_restarts.remove(process_id);
            if (self.processes.get(process_id)) |proc| {
                self.doRestart(process_id, proc) catch {
                    proc.status = .errored;
                };
            }
        }
    }

    /// Drain any pending pipe fds that need to be registered in the event loop.
    /// Returns the fds. Caller is responsible for registering them.
    pub fn drainPendingPipeFds(self: *Self) []posix.fd_t {
        const fds = self.pending_pipe_fds.toOwnedSlice(self.allocator) catch return &[_]posix.fd_t{};
        return fds;
    }

    /// Check pending kills and send SIGKILL if timeout expired
    pub fn checkPendingKills(self: *Self) void {
        const now: u64 = @intCast(std.time.milliTimestamp());

        var to_kill: std.ArrayList(u32) = .{};
        defer to_kill.deinit(self.allocator);

        var it = self.pending_kills.iterator();
        while (it.next()) |entry| {
            if (now >= entry.value_ptr.*) {
                to_kill.append(self.allocator, entry.key_ptr.*) catch continue;
            }
        }

        for (to_kill.items) |process_id| {
            if (self.processes.get(process_id)) |proc| {
                if (proc.status == .running or proc.status == .stopped) {
                    signals.sendSignal(proc.pid, signals.SIGKILL) catch {};
                }
            }
            _ = self.pending_kills.remove(process_id);
        }
    }

    /// Get info about all processes
    pub fn listProcesses(self: *Self) ![]ProcessInfo {
        var list: std.ArrayList(ProcessInfo) = .{};

        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            const proc = proc_ptr.*;
            // Update uptime
            if (proc.status == .running) {
                const now: u64 = @intCast(std.time.milliTimestamp());
                proc.uptime_ms = now - proc.start_time_ms;
            }
            try list.append(self.allocator, ProcessInfo{
                .id = proc.id,
                .name = proc.name,
                .pid = proc.pid,
                .status = proc.status,
                .memory_bytes = proc.memory_bytes,
                .uptime_ms = proc.uptime_ms,
                .restart_count = proc.restart_count,
                .start_time_ms = proc.start_time_ms,
                .config = proc.config,
            });
        }

        return list.toOwnedSlice(self.allocator);
    }

    pub fn freeProcessList(self: *Self, list: []ProcessInfo) void {
        self.allocator.free(list);
    }

    pub fn getProcess(self: *Self, process_id: u32) ?*ProcessInfo {
        return self.processes.get(process_id);
    }

    /// Stop all running processes (graceful shutdown)
    pub fn stopAll(self: *Self) void {
        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            const proc = proc_ptr.*;
            if (proc.status == .running) {
                signals.sendSignal(proc.pid, signals.SIGTERM) catch {};
                proc.status = .stopped;
            }
        }
    }

    pub fn processCount(self: *Self) u32 {
        return @intCast(self.processes.count());
    }

    /// Public restart: stop + restart a process by id. Used by IPC restart command.
    /// Returns pipe fds for event loop registration.
    pub fn restartProcess(self: *Self, process_id: u32) !struct { stdout_fd: posix.fd_t, stderr_fd: posix.fd_t } {
        const proc = self.processes.get(process_id) orelse return error.ProcessNotFound;

        // If running, stop it first
        if (proc.status == .running) {
            signals.sendSignal(proc.pid, signals.SIGTERM) catch {};
            // Brief wait for child to exit (non-blocking reap)
            var status: c_int = 0;
            _ = std.c.waitpid(proc.pid, &status, std.c.W.NOHANG);
            _ = self.pid_to_id.remove(proc.pid);
        }

        _ = self.pending_kills.remove(process_id);

        // Restart
        try self.doRestart(process_id, proc);

        // Return the pipe fds (last two added)
        const fds = self.pending_pipe_fds.items;
        if (fds.len >= 2) {
            const stdout_fd = fds[fds.len - 2];
            const stderr_fd = fds[fds.len - 1];
            return .{ .stdout_fd = stdout_fd, .stderr_fd = stderr_fd };
        }
        return error.RestartFailed;
    }

    /// Update resource usage (RSS memory) for all running processes.
    /// Should be called periodically (every ~2 seconds).
    pub fn updateResourceUsage(self: *Self) void {
        const now: u64 = @intCast(std.time.milliTimestamp());
        if (now - self.last_resource_check_ms < 2000) return;
        self.last_resource_check_ms = now;

        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            const proc = proc_ptr.*;
            if (proc.status != .running) continue;

            const rss = getProcessRss(proc.pid);
            if (rss > 0) {
                proc.memory_bytes = rss;
                // max_memory_restart check
                if (proc.config.max_memory_restart > 0 and rss > proc.config.max_memory_restart) {
                    self.doRestart(proc.id, proc) catch {
                        proc.status = .errored;
                    };
                }
            }
        }
    }

    /// Detect interpreter for a script file based on shebang or extension.
    fn detectInterpreter(self: *Self, script: []const u8, cwd: []const u8) ?[*:0]const u8 {
        _ = self;
        // Try shebang detection: read first 256 bytes of the file
        const shebang_interp = detectShebang(script, cwd);
        if (shebang_interp) |interp| return interp;

        // Extension-based detection
        if (std.mem.endsWith(u8, script, ".py")) return "python3";
        if (std.mem.endsWith(u8, script, ".js")) return "node";
        if (std.mem.endsWith(u8, script, ".mjs")) return "node";
        if (std.mem.endsWith(u8, script, ".cjs")) return "node";
        if (std.mem.endsWith(u8, script, ".ts")) return "npx";
        if (std.mem.endsWith(u8, script, ".tsx")) return "npx";
        if (std.mem.endsWith(u8, script, ".rb")) return "ruby";
        if (std.mem.endsWith(u8, script, ".sh")) return "/bin/sh";

        return null;
    }

    /// Get all process configs for state saving
    pub fn getAllConfigs(self: *Self) ![]*ProcessInfo {
        var list: std.ArrayList(*ProcessInfo) = .{};
        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            try list.append(self.allocator, proc_ptr.*);
        }
        return list.toOwnedSlice(self.allocator);
    }

    /// Setup watcher and cron for a process based on its config.
    fn setupWatchAndCron(self: *Self, process_id: u32) void {
        const proc = self.processes.get(process_id) orelse return;

        // Setup file watcher
        if (proc.config.watch) {
            const watcher = self.allocator.create(Watcher) catch return;
            watcher.* = Watcher.init(
                self.allocator,
                proc.config.watch_paths,
                proc.config.watch_ignore,
                proc.config.watch_delay_ms,
            );
            watcher.setup(proc.config.cwd) catch {
                watcher.deinit();
                self.allocator.destroy(watcher);
                return;
            };
            // Remove old watcher if exists
            if (self.watchers.get(process_id)) |old_w| {
                var old = old_w;
                old.deinit();
                self.allocator.destroy(old_w);
            }
            self.watchers.put(process_id, watcher) catch {
                watcher.deinit();
                self.allocator.destroy(watcher);
            };
        }

        // Setup cron expression
        if (proc.config.cron_restart) |cron_str| {
            const expr = CronExpr.parse(cron_str) catch return;
            self.cron_exprs.put(process_id, expr) catch {};
        }
    }

    /// Check all file watchers for changes and restart processes.
    pub fn checkWatchers(self: *Self) void {
        var to_restart: std.ArrayList(u32) = .{};
        defer to_restart.deinit(self.allocator);

        var it = self.watchers.iterator();
        while (it.next()) |entry| {
            const watcher = entry.value_ptr.*;
            if (watcher.checkForChanges()) {
                to_restart.append(self.allocator, entry.key_ptr.*) catch {};
            }
        }

        for (to_restart.items) |process_id| {
            if (self.processes.get(process_id)) |proc| {
                if (proc.status == .running) {
                    self.doRestart(process_id, proc) catch {
                        proc.status = .errored;
                    };
                }
            }
        }
    }

    /// Check cron expressions and restart matching processes.
    pub fn checkCronRestarts(self: *Self) void {
        if (self.cron_exprs.count() == 0) return;

        // Get current time via libc localtime
        const now_s: c_long = @intCast(@divTrunc(std.time.milliTimestamp(), 1000));
        const tm = localtime(&now_s) orelse return;

        // Only check once per minute
        const current_minute: i32 = @as(i32, tm.tm_hour) * 60 + tm.tm_min;
        if (current_minute == self.last_cron_minute) return;
        self.last_cron_minute = current_minute;

        const minute: u8 = @intCast(tm.tm_min);
        const hour: u8 = @intCast(tm.tm_hour);
        const day: u8 = @intCast(tm.tm_mday);
        const month: u8 = @intCast(tm.tm_mon + 1); // tm_mon is 0-based
        const weekday: u8 = @intCast(tm.tm_wday); // 0=Sunday

        var to_restart: std.ArrayList(u32) = .{};
        defer to_restart.deinit(self.allocator);

        var it = self.cron_exprs.iterator();
        while (it.next()) |entry| {
            if (entry.value_ptr.matches(minute, hour, day, month, weekday)) {
                to_restart.append(self.allocator, entry.key_ptr.*) catch {};
            }
        }

        for (to_restart.items) |process_id| {
            if (self.processes.get(process_id)) |proc| {
                if (proc.status == .running) {
                    self.doRestart(process_id, proc) catch {
                        proc.status = .errored;
                    };
                }
            }
        }
    }

    /// Check IPC channels for wait_ready "ready" messages.
    pub fn checkWaitReady(self: *Self) void {
        var it = self.ipc_channels.iterator();
        while (it.next()) |entry| {
            const process_id = entry.key_ptr.*;
            const proc = self.processes.get(process_id) orelse continue;

            if (proc.status != .starting) continue;

            const ch = entry.value_ptr;

            // Try to read a message (non-blocking)
            if (IpcChannel.readMessage(self.allocator, ch.parent_fd) catch null) |msg| {
                defer self.allocator.free(msg);
                proc.status = .running;
                continue;
            }

            // Check listen_timeout
            const now: u64 = @intCast(std.time.milliTimestamp());
            if (now - proc.start_time_ms >= proc.config.listen_timeout_ms) {
                proc.status = .running; // timeout → assume ready
            }
        }
    }

    /// Scale a cluster to target_count instances. Returns started/stopped counts.
    /// New instances' pipe fds are stored in pending_pipe_fds for event loop registration.
    pub fn scaleCluster(self: *Self, base_name: []const u8, target_count: u32) !struct { started: u32, stopped: u32 } {
        // Find all instances matching base_name
        var instance_ids: std.ArrayList(struct { proc_id: u32, inst_id: u32 }) = .{};
        defer instance_ids.deinit(self.allocator);

        var template_proc: ?*ProcessInfo = null;
        var max_instance_id: u32 = 0;

        var it = self.processes.valueIterator();
        while (it.next()) |proc_ptr| {
            const proc = proc_ptr.*;
            if (matchesBaseName(proc.name, base_name)) {
                try instance_ids.append(self.allocator, .{ .proc_id = proc.id, .inst_id = proc.instance_id });
                if (template_proc == null) template_proc = proc;
                if (proc.instance_id > max_instance_id) max_instance_id = proc.instance_id;
            }
        }

        const current: u32 = @intCast(instance_ids.items.len);
        var started: u32 = 0;
        var stopped: u32 = 0;

        if (target_count > current) {
            // Scale up: start new instances
            const template = template_proc orelse return error.ProcessNotFound;
            var next_inst = max_instance_id + 1;
            // If scaling from fork mode (single instance, no :N suffix), rename it to :0
            if (current == 1 and std.mem.indexOfScalar(u8, template.name, ':') == null) {
                const new_name = try std.fmt.allocPrint(self.allocator, "{s}:0", .{base_name});
                self.allocator.free(template.name);
                template.name = new_name;
                const new_cfg_name = try self.allocator.dupe(u8, new_name);
                self.allocator.free(template.config.name);
                template.config.name = new_cfg_name;
                template.config.instance_id = 0;
                template.config.instances = target_count;
                template.instance_id = 0;
                next_inst = 1;
            }

            var i: u32 = 0;
            while (i < target_count - current) : (i += 1) {
                const inst_name = std.fmt.allocPrint(self.allocator, "{s}:{d}", .{ base_name, next_inst }) catch continue;
                defer self.allocator.free(inst_name);

                var cfg = template.config;
                cfg.name = inst_name;
                cfg.instance_id = next_inst;
                cfg.instances = target_count;

                const result = self.startProcess(cfg) catch continue;
                try self.pending_pipe_fds.append(self.allocator, result.stdout_fd);
                try self.pending_pipe_fds.append(self.allocator, result.stderr_fd);
                started += 1;
                next_inst += 1;
            }

            // Update instances count on all existing instances
            var it2 = self.processes.valueIterator();
            while (it2.next()) |proc_ptr| {
                const proc = proc_ptr.*;
                if (matchesBaseName(proc.name, base_name)) {
                    proc.config.instances = target_count;
                }
            }
        } else if (target_count < current) {
            // Scale down: stop instances with highest instance_id first
            var to_stop = current - target_count;
            while (to_stop > 0) {
                var max_id: u32 = 0;
                var max_proc_id: u32 = 0;
                var found = false;

                var it2 = self.processes.valueIterator();
                while (it2.next()) |proc_ptr| {
                    const proc = proc_ptr.*;
                    if (matchesBaseName(proc.name, base_name) and
                        (proc.status == .running or proc.status == .starting))
                    {
                        if (!found or proc.instance_id > max_id) {
                            max_id = proc.instance_id;
                            max_proc_id = proc.id;
                            found = true;
                        }
                    }
                }

                if (!found) break;

                self.stopProcess(max_proc_id, signals.SIGTERM, 5000) catch {};
                stopped += 1;
                to_stop -= 1;
            }

            // Update instances count on remaining
            var it3 = self.processes.valueIterator();
            while (it3.next()) |proc_ptr| {
                const proc = proc_ptr.*;
                if (matchesBaseName(proc.name, base_name)) {
                    proc.config.instances = target_count;
                }
            }
        }

        return .{ .started = started, .stopped = stopped };
    }
};

// C tm struct for localtime
const CTm = extern struct {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
};

extern "c" fn localtime(timer: *const c_long) ?*const CTm;
extern "c" fn setenv(name: [*:0]const u8, value: [*:0]const u8, overwrite: c_int) c_int;

/// Check if a process name matches a base name (exact or "base:N" pattern).
fn matchesBaseName(name: []const u8, base_name: []const u8) bool {
    if (std.mem.eql(u8, name, base_name)) return true;
    if (name.len > base_name.len + 1 and
        std.mem.startsWith(u8, name, base_name) and
        name[base_name.len] == ':')
    {
        const suffix = name[base_name.len + 1 ..];
        _ = std.fmt.parseInt(u32, suffix, 10) catch return false;
        return true;
    }
    return false;
}

/// Set VELOS_INSTANCE_ID and NODE_APP_INSTANCE env vars in child process
fn setInstanceEnv(instance_id: u32) void {
    var id_buf: [20]u8 = undefined;
    const id_str = std.fmt.bufPrint(&id_buf, "{d}", .{instance_id}) catch return;
    var id_val: [21]u8 = [_]u8{0} ** 21;
    @memcpy(id_val[0..id_str.len], id_str);
    _ = setenv("VELOS_INSTANCE_ID", @ptrCast(&id_val), 1);
    _ = setenv("NODE_APP_INSTANCE", @ptrCast(&id_val), 1);
}

/// Set VELOS_IPC_FD environment variable in child process before exec
fn setIpcEnv(child_fd: posix.fd_t) void {
    var fd_buf: [20]u8 = undefined;
    const fd_str = std.fmt.bufPrint(&fd_buf, "{d}", .{child_fd}) catch return;
    var fd_val: [21]u8 = [_]u8{0} ** 21;
    @memcpy(fd_val[0..fd_str.len], fd_str);
    _ = setenv("VELOS_IPC_FD", @ptrCast(&fd_val), 1);
}

fn getProcessRss(pid: posix.pid_t) u64 {
    if (comptime builtin.os.tag == .macos) {
        return getProcessRssMacos(pid);
    } else if (comptime builtin.os.tag == .linux) {
        return getProcessRssLinux(pid);
    }
    return 0;
}

// macOS-specific RSS monitoring (only compiled on macOS)
const macos_rss = if (builtin.os.tag == .macos) struct {
    extern "c" fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *anyopaque) c_int;
    const RUSAGE_INFO_V0 = 0;
    const RusageInfoV0 = extern struct {
        ri_uuid: [16]u8 = std.mem.zeroes([16]u8),
        ri_user_time: u64 = 0,
        ri_system_time: u64 = 0,
        ri_pkg_idle_wkups: u64 = 0,
        ri_interrupt_wkups: u64 = 0,
        ri_pageins: u64 = 0,
        ri_wired_size: u64 = 0,
        ri_resident_size: u64 = 0,
        ri_phys_footprint: u64 = 0,
        ri_proc_start_abstime: u64 = 0,
        ri_proc_exit_abstime: u64 = 0,
    };
} else struct {};

fn getProcessRssMacos(pid: posix.pid_t) u64 {
    if (comptime builtin.os.tag != .macos) return 0;
    var rusage: macos_rss.RusageInfoV0 = .{};
    const ret = macos_rss.proc_pid_rusage(@intCast(pid), macos_rss.RUSAGE_INFO_V0, @ptrCast(&rusage));
    if (ret != 0) return 0;
    return rusage.ri_resident_size;
}

fn getProcessRssLinux(pid: posix.pid_t) u64 {
    // Read /proc/[pid]/statm — second field is RSS in pages
    var path_buf: [64]u8 = undefined;
    const path = std.fmt.bufPrint(&path_buf, "/proc/{d}/statm", .{pid}) catch return 0;
    const file = std.fs.openFileAbsolute(path, .{}) catch return 0;
    defer file.close();
    var buf: [256]u8 = undefined;
    const n = file.readAll(&buf) catch return 0;
    const content = buf[0..n];
    // Format: size resident shared text lib data dt
    var it = std.mem.splitScalar(u8, content, ' ');
    _ = it.next(); // skip size
    const rss_str = it.next() orelse return 0;
    const rss_pages = std.fmt.parseInt(u64, rss_str, 10) catch return 0;
    return rss_pages * 4096; // assume 4K pages
}

fn detectShebang(script: []const u8, cwd: []const u8) ?[*:0]const u8 {
    // Try absolute path first, then relative to cwd
    const file = std.fs.openFileAbsolute(script, .{}) catch blk: {
        // Try relative to cwd
        const dir = std.fs.openDirAbsolute(cwd, .{}) catch return null;
        break :blk dir.openFile(script, .{}) catch return null;
    };
    defer file.close();

    var buf: [256]u8 = undefined;
    const n = file.readAll(&buf) catch return null;
    if (n < 2) return null;
    if (buf[0] != '#' or buf[1] != '!') return null;

    // Extract interpreter path from shebang line
    const line_end = std.mem.indexOfScalar(u8, buf[2..n], '\n') orelse (n - 2);
    const shebang = std.mem.trim(u8, buf[2..][0..line_end], " \t\r");

    // Handle "#!/usr/bin/env python3" style
    if (std.mem.startsWith(u8, shebang, "/usr/bin/env ")) {
        const rest = std.mem.trim(u8, shebang["/usr/bin/env ".len..], " \t");
        // Get just the interpreter name (first word)
        const space = std.mem.indexOfScalar(u8, rest, ' ') orelse rest.len;
        const interp = rest[0..space];
        if (std.mem.eql(u8, interp, "python3")) return "python3";
        if (std.mem.eql(u8, interp, "python")) return "python3";
        if (std.mem.eql(u8, interp, "node")) return "node";
        if (std.mem.eql(u8, interp, "ruby")) return "ruby";
        if (std.mem.eql(u8, interp, "bash")) return "/bin/bash";
        if (std.mem.eql(u8, interp, "sh")) return "/bin/sh";
    }

    // Direct path shebangs
    if (std.mem.eql(u8, shebang, "/bin/sh") or std.mem.startsWith(u8, shebang, "/bin/sh ")) return "/bin/sh";
    if (std.mem.eql(u8, shebang, "/bin/bash") or std.mem.startsWith(u8, shebang, "/bin/bash ")) return "/bin/bash";

    return null;
}

fn needsTsxArg(script: []const u8) bool {
    return std.mem.endsWith(u8, script, ".ts") or std.mem.endsWith(u8, script, ".tsx");
}

fn setNonBlocking(fd: posix.fd_t) void {
    const flags = std.c.fcntl(fd, std.c.F.GETFL);
    _ = std.c.fcntl(fd, std.c.F.SETFL, @as(c_int, flags) | @as(c_int, @bitCast(std.c.O{ .NONBLOCK = true })));
}
