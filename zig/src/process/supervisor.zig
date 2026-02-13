const std = @import("std");
const posix = std.posix;
const signals = @import("signals.zig");
const LogCollector = @import("../log/collector.zig").LogCollector;

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

    pub fn init(allocator: std.mem.Allocator, log_collector: *LogCollector) Self {
        return Self{
            .processes = std.AutoHashMap(u32, *ProcessInfo).init(allocator),
            .pid_to_id = std.AutoHashMap(posix.pid_t, u32).init(allocator),
            .next_id = 1,
            .log_collector = log_collector,
            .allocator = allocator,
            .pending_kills = std.AutoHashMap(u32, u64).init(allocator),
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
            self.allocator.destroy(proc);
        }
        self.processes.deinit();
        self.pid_to_id.deinit();
        self.pending_kills.deinit();
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
            // Auto-detect: check file extension
            if (std.mem.endsWith(u8, config.script, ".py")) {
                try argv_list.append(self.allocator, "python3");
            } else if (std.mem.endsWith(u8, config.script, ".js")) {
                try argv_list.append(self.allocator, "node");
            } else if (std.mem.endsWith(u8, config.script, ".rb")) {
                try argv_list.append(self.allocator, "ruby");
            } else if (std.mem.endsWith(u8, config.script, ".sh")) {
                try argv_list.append(self.allocator, "/bin/sh");
            } else {
                // Direct execution
            }
        }

        const script_z = try self.allocator.dupeZ(u8, config.script);
        defer self.allocator.free(script_z);
        try argv_list.append(self.allocator, script_z);
        try argv_list.append(self.allocator, null); // null terminator

        const cwd_z = try self.allocator.dupeZ(u8, config.cwd);
        defer self.allocator.free(cwd_z);

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
            },
        };

        try self.processes.put(id, proc);
        try self.pid_to_id.put(pid, id);

        // Register with log collector
        try self.log_collector.addProcess(id, config.name, stdout_pipe[0], stderr_pipe[0]);

        return .{
            .id = id,
            .stdout_fd = stdout_pipe[0],
            .stderr_fd = stderr_pipe[0],
        };
    }

    /// Initiate graceful stop: send SIGTERM and set kill deadline.
    pub fn stopProcess(self: *Self, process_id: u32, sig: u8, timeout_ms: u32) !void {
        const proc = self.processes.get(process_id) orelse return error.ProcessNotFound;
        if (proc.status != .running) return;

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

        self.allocator.free(proc.name);
        self.allocator.free(proc.config.name);
        self.allocator.free(proc.config.script);
        self.allocator.free(proc.config.cwd);
        if (proc.config.interpreter) |interp| self.allocator.free(interp);
        self.allocator.destroy(proc);
        _ = self.processes.remove(process_id);
    }

    /// Called when SIGCHLD is received. Reaps children and updates status.
    pub fn handleSigchld(self: *Self) !void {
        const reaped = try signals.reapChildren(self.allocator);
        defer self.allocator.free(reaped);

        for (reaped) |reap| {
            const process_id = self.pid_to_id.get(reap.pid) orelse continue;
            const proc = self.processes.get(process_id) orelse continue;

            if (reap.signaled or reap.exit_code != 0) {
                proc.status = .errored;
            } else {
                proc.status = .stopped;
            }

            _ = self.pending_kills.remove(process_id);
            _ = self.pid_to_id.remove(reap.pid);
        }
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
};

fn setNonBlocking(fd: posix.fd_t) void {
    const flags = std.c.fcntl(fd, std.c.F.GETFL);
    _ = std.c.fcntl(fd, std.c.F.SETFL, @as(c_int, flags) | @as(c_int, @bitCast(std.c.O{ .NONBLOCK = true })));
}
