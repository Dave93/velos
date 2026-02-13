const std = @import("std");
const posix = std.posix;
const RingBuffer = @import("ring_buffer.zig").RingBuffer;
const LogWriter = @import("writer.zig").LogWriter;

/// Collects output from child process pipes and routes to ring buffer + file writer.
pub const LogCollector = struct {
    const Self = @This();
    const DEFAULT_RING_SIZE = 1000;

    /// Per-process log state
    pub const ProcessLog = struct {
        process_id: u32,
        name: []const u8, // owned
        stdout_fd: ?posix.fd_t,
        stderr_fd: ?posix.fd_t,
        ring: RingBuffer,
    };

    processes: std.AutoHashMap(u32, *ProcessLog), // process_id -> log state
    fd_to_process: std.AutoHashMap(posix.fd_t, FdInfo),
    writer: LogWriter,
    allocator: std.mem.Allocator,

    const FdInfo = struct {
        process_id: u32,
        stream: u8, // 0=stdout, 1=stderr
    };

    pub fn init(allocator: std.mem.Allocator, log_dir: []const u8) Self {
        return Self{
            .processes = std.AutoHashMap(u32, *ProcessLog).init(allocator),
            .fd_to_process = std.AutoHashMap(posix.fd_t, FdInfo).init(allocator),
            .writer = LogWriter.init(allocator, log_dir),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        var it = self.processes.valueIterator();
        while (it.next()) |proc_log_ptr| {
            const proc_log = proc_log_ptr.*;
            proc_log.ring.deinit();
            self.allocator.free(proc_log.name);
            self.allocator.destroy(proc_log);
        }
        self.processes.deinit();
        self.fd_to_process.deinit();
    }

    /// Register a new process for log collection
    pub fn addProcess(
        self: *Self,
        process_id: u32,
        name: []const u8,
        stdout_fd: ?posix.fd_t,
        stderr_fd: ?posix.fd_t,
    ) !void {
        const proc_log = try self.allocator.create(ProcessLog);
        proc_log.* = ProcessLog{
            .process_id = process_id,
            .name = try self.allocator.dupe(u8, name),
            .stdout_fd = stdout_fd,
            .stderr_fd = stderr_fd,
            .ring = try RingBuffer.init(self.allocator, DEFAULT_RING_SIZE),
        };

        try self.processes.put(process_id, proc_log);

        if (stdout_fd) |fd| {
            try self.fd_to_process.put(fd, FdInfo{ .process_id = process_id, .stream = 0 });
        }
        if (stderr_fd) |fd| {
            try self.fd_to_process.put(fd, FdInfo{ .process_id = process_id, .stream = 1 });
        }
    }

    /// Remove a process from log collection. Closes pipe fds.
    pub fn removeProcess(self: *Self, process_id: u32) void {
        const proc_log = self.processes.get(process_id) orelse return;

        if (proc_log.stdout_fd) |fd| {
            _ = self.fd_to_process.remove(fd);
            posix.close(fd);
        }
        if (proc_log.stderr_fd) |fd| {
            _ = self.fd_to_process.remove(fd);
            posix.close(fd);
        }

        proc_log.ring.deinit();
        self.allocator.free(proc_log.name);
        self.allocator.destroy(proc_log);
        _ = self.processes.remove(process_id);
    }

    /// Called when data is available on a pipe fd. Reads and processes it.
    pub fn handlePipeData(self: *Self, fd: posix.fd_t) !void {
        const info = self.fd_to_process.get(fd) orelse return;
        const proc_log = self.processes.get(info.process_id) orelse return;

        var buf: [4096]u8 = undefined;
        const n = posix.read(fd, &buf) catch |err| {
            if (err == error.WouldBlock) return;
            return err;
        };
        if (n == 0) return; // EOF

        const data = buf[0..n];

        // Split into lines and process each
        var start: usize = 0;
        for (data, 0..) |byte, i| {
            if (byte == '\n') {
                const line = data[start..i];
                self.processLine(proc_log, info.stream, line) catch {};
                start = i + 1;
            }
        }
        // Handle remaining data (no newline yet)
        if (start < data.len) {
            const line = data[start..];
            self.processLine(proc_log, info.stream, line) catch {};
        }
    }

    fn processLine(self: *Self, proc_log: *ProcessLog, stream: u8, line: []const u8) !void {
        if (line.len == 0) return;

        const timestamp_ms = getTimestampMs();
        const level: u8 = if (stream == 1) 3 else 1; // stderr = error, stdout = info

        // Write to ring buffer
        try proc_log.ring.push(timestamp_ms, level, stream, line);

        // Write to file
        self.writer.writeLine(proc_log.name, stream, line) catch {};
    }

    /// Read last N log lines for a process
    pub fn readLast(self: *Self, process_id: u32, n: u32) ![]RingBuffer.Entry {
        const proc_log = self.processes.get(process_id) orelse return error.ProcessNotFound;
        return try proc_log.ring.readLast(n);
    }

    pub fn freeEntries(self: *Self, entries: []RingBuffer.Entry) void {
        self.allocator.free(entries);
    }

    /// Close a specific pipe fd (on HUP)
    pub fn closePipe(self: *Self, fd: posix.fd_t) void {
        const info = self.fd_to_process.get(fd) orelse return;
        const proc_log = self.processes.get(info.process_id) orelse return;

        if (proc_log.stdout_fd) |sfd| {
            if (sfd == fd) proc_log.stdout_fd = null;
        }
        if (proc_log.stderr_fd) |sfd| {
            if (sfd == fd) proc_log.stderr_fd = null;
        }

        _ = self.fd_to_process.remove(fd);
        posix.close(fd);
    }

    /// Get the fd -> process mapping (for event loop registration)
    pub fn getFdInfo(self: *Self, fd: posix.fd_t) ?FdInfo {
        return self.fd_to_process.get(fd);
    }
};

fn getTimestampMs() u64 {
    const ts = std.time.milliTimestamp();
    return @intCast(ts);
}
