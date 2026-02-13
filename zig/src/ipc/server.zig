const std = @import("std");
const posix = std.posix;
const protocol = @import("protocol.zig");
const pal = @import("../platform/pal.zig");
const Supervisor = @import("../process/supervisor.zig").Supervisor;
const ProcessConfig = @import("../process/supervisor.zig").ProcessConfig;
const ProcessInfo = @import("../process/supervisor.zig").ProcessInfo;
const ProcessStatus = @import("../process/supervisor.zig").ProcessStatus;
const LogCollector = @import("../log/collector.zig").LogCollector;
const Persistence = @import("../state/persistence.zig").Persistence;

pub const IpcServer = struct {
    const Self = @This();
    const MAX_CLIENTS = 64;

    /// Per-client connection state
    const ClientState = struct {
        fd: posix.fd_t,
        recv_buf: std.ArrayList(u8),
    };

    listen_fd: posix.fd_t,
    socket_path: []const u8,
    clients: std.AutoHashMap(posix.fd_t, *ClientState),
    event_loop: *pal.Pal,
    supervisor: *Supervisor,
    log_collector: *LogCollector,
    persistence: ?*Persistence,
    allocator: std.mem.Allocator,
    shutdown_requested: bool,

    pub fn init(
        allocator: std.mem.Allocator,
        socket_path: []const u8,
        event_loop: *pal.Pal,
        supervisor: *Supervisor,
        log_collector: *LogCollector,
    ) !Self {
        // Remove existing socket if any
        std.fs.deleteFileAbsolute(socket_path) catch {};

        // Create and bind Unix socket
        const listen_fd = try posix.socket(posix.AF.UNIX, posix.SOCK.STREAM, 0);

        var addr = std.net.Address.initUnix(socket_path) catch return error.InvalidSocketPath;
        try posix.bind(listen_fd, &addr.any, addr.getOsSockLen());

        // Security: restrict socket to owner only (0600)
        const path_z: [*:0]const u8 = @ptrCast(socket_path.ptr);
        _ = std.c.chmod(path_z, 0o600);

        try posix.listen(listen_fd, 16);

        // Register listen socket in event loop
        try event_loop.addFd(listen_fd, .ipc_accept);

        return Self{
            .listen_fd = listen_fd,
            .socket_path = socket_path,
            .clients = std.AutoHashMap(posix.fd_t, *ClientState).init(allocator),
            .event_loop = event_loop,
            .supervisor = supervisor,
            .log_collector = log_collector,
            .persistence = null,
            .allocator = allocator,
            .shutdown_requested = false,
        };
    }

    pub fn deinit(self: *Self) void {
        // Close all client connections
        var it = self.clients.valueIterator();
        while (it.next()) |client_ptr| {
            const client = client_ptr.*;
            posix.close(client.fd);
            client.recv_buf.deinit(self.allocator);
            self.allocator.destroy(client);
        }
        self.clients.deinit();

        posix.close(self.listen_fd);

        // Remove socket file
        std.fs.deleteFileAbsolute(self.socket_path) catch {};
    }

    /// Accept a new client connection
    pub fn acceptClient(self: *Self) !void {
        const client_fd = try posix.accept(self.listen_fd, null, null, 0);

        // Set non-blocking
        const flags = std.c.fcntl(client_fd, std.c.F.GETFL);
        _ = std.c.fcntl(client_fd, std.c.F.SETFL, @as(c_int, flags) | @as(c_int, @bitCast(std.c.O{ .NONBLOCK = true })));

        const client = try self.allocator.create(ClientState);
        client.* = ClientState{
            .fd = client_fd,
            .recv_buf = .{},
        };

        try self.clients.put(client_fd, client);
        try self.event_loop.addFd(client_fd, .ipc_read);
    }

    /// Handle data from a client fd
    pub fn handleClientData(self: *Self, fd: posix.fd_t) !void {
        const client = self.clients.get(fd) orelse return;

        var buf: [4096]u8 = undefined;
        const n = posix.read(fd, &buf) catch |err| {
            if (err == error.WouldBlock) return;
            self.removeClient(fd);
            return;
        };
        if (n == 0) {
            self.removeClient(fd);
            return;
        }

        try client.recv_buf.appendSlice(self.allocator, buf[0..n]);

        // Try to parse complete messages
        while (try self.tryParseMessage(client)) {}
    }

    fn tryParseMessage(self: *Self, client: *ClientState) !bool {
        const data = client.recv_buf.items;
        if (data.len < protocol.HEADER_SIZE) return false;

        const payload_len = protocol.decodeHeader(data[0..protocol.HEADER_SIZE]) catch {
            // Invalid header - disconnect client
            self.removeClient(client.fd);
            return false;
        };

        const total_len = protocol.HEADER_SIZE + payload_len;
        if (data.len < total_len) return false;

        // Extract payload
        const payload = data[protocol.HEADER_SIZE..total_len];
        const request = protocol.decodeRequest(payload) catch {
            self.removeClient(client.fd);
            return false;
        };

        // Handle the request
        self.handleRequest(client.fd, request) catch {};

        // Remove consumed bytes from buffer
        const remaining = data.len - total_len;
        if (remaining > 0) {
            std.mem.copyForwards(u8, client.recv_buf.items[0..remaining], data[total_len..]);
        }
        client.recv_buf.shrinkRetainingCapacity(remaining);

        return remaining >= protocol.HEADER_SIZE;
    }

    fn handleRequest(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        switch (request.command) {
            .ping => try self.handlePing(client_fd, request.id),
            .process_start => try self.handleProcessStart(client_fd, request),
            .process_stop => try self.handleProcessStop(client_fd, request),
            .process_restart => try self.handleProcessRestart(client_fd, request),
            .process_delete => try self.handleProcessDelete(client_fd, request),
            .process_list => try self.handleProcessList(client_fd, request.id),
            .process_info => try self.handleProcessInfo(client_fd, request),
            .process_scale => try self.handleProcessScale(client_fd, request),
            .log_read => try self.handleLogRead(client_fd, request),
            .state_save => try self.handleStateSave(client_fd, request.id),
            .state_load => try self.handleStateLoad(client_fd, request.id),
            .shutdown => try self.handleShutdown(client_fd, request.id),
            _ => try self.sendError(client_fd, request.id, "unknown command"),
        }
    }

    fn handlePing(self: *Self, client_fd: posix.fd_t, req_id: u32) !void {
        const payload = "pong";
        try self.sendResponse(client_fd, req_id, .ok, payload);
    }

    fn handleProcessStart(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Parse config from payload:
        // name(string) + script(string) + cwd(string) + interpreter(string, empty=null)
        // + kill_timeout(u32) + autorestart(u8) + max_restarts(i32) + min_uptime_ms(u64)
        // + restart_delay_ms(u32) + exp_backoff(u8)
        const data = request.payload;
        var off: usize = 0;

        const name_r = protocol.readString(data, off);
        off = name_r.next;
        const script_r = protocol.readString(data, off);
        off = script_r.next;
        const cwd_r = protocol.readString(data, off);
        off = cwd_r.next;
        const interp_r = protocol.readString(data, off);
        off = interp_r.next;
        const timeout_r = protocol.readU32(data, off);
        off = timeout_r.next;
        const autorestart_r = protocol.readU8(data, off);
        off = autorestart_r.next;

        // New Phase 3 fields (with defaults for backward compatibility)
        const has_extended = off < data.len;
        const max_restarts_r = protocol.readI32(data, off);
        off = max_restarts_r.next;
        const min_uptime_r = protocol.readU64(data, off);
        off = min_uptime_r.next;
        const restart_delay_r = protocol.readU32(data, off);
        off = restart_delay_r.next;
        const exp_backoff_r = protocol.readU8(data, off);
        off = exp_backoff_r.next;

        // Phase 3 extended fields (batch 2)
        const has_extended2 = off < data.len;
        const max_mem_r = protocol.readU64(data, off);
        off = max_mem_r.next;
        const watch_r = protocol.readU8(data, off);
        off = watch_r.next;
        const watch_delay_r = protocol.readU32(data, off);
        off = watch_delay_r.next;
        const watch_paths_r = protocol.readString(data, off);
        off = watch_paths_r.next;
        const watch_ignore_r = protocol.readString(data, off);
        off = watch_ignore_r.next;
        const cron_r = protocol.readString(data, off);
        off = cron_r.next;
        const wait_ready_r = protocol.readU8(data, off);
        off = wait_ready_r.next;
        const listen_timeout_r = protocol.readU32(data, off);
        off = listen_timeout_r.next;
        const shutdown_msg_r = protocol.readU8(data, off);
        off = shutdown_msg_r.next;

        // Phase 6: cluster mode instances field
        const has_extended3 = off < data.len;
        const instances_r = protocol.readU32(data, off);
        const instances: u32 = if (has_extended3 and instances_r.val > 0) instances_r.val else 1;

        const config = ProcessConfig{
            .name = name_r.val,
            .script = script_r.val,
            .cwd = cwd_r.val,
            .interpreter = if (interp_r.val.len == 0) null else interp_r.val,
            .kill_timeout_ms = if (timeout_r.val == 0) 5000 else timeout_r.val,
            .autorestart = autorestart_r.val != 0,
            .max_restarts = if (has_extended) max_restarts_r.val else 15,
            .min_uptime_ms = if (has_extended and min_uptime_r.val != 0) min_uptime_r.val else 1000,
            .restart_delay_ms = restart_delay_r.val,
            .exp_backoff = exp_backoff_r.val != 0,
            .max_memory_restart = if (has_extended2) max_mem_r.val else 0,
            .watch = if (has_extended2) watch_r.val != 0 else false,
            .watch_delay_ms = if (has_extended2 and watch_delay_r.val != 0) watch_delay_r.val else 1000,
            .watch_paths = if (has_extended2 and watch_paths_r.val.len > 0) watch_paths_r.val else null,
            .watch_ignore = if (has_extended2 and watch_ignore_r.val.len > 0) watch_ignore_r.val else null,
            .cron_restart = if (has_extended2 and cron_r.val.len > 0) cron_r.val else null,
            .wait_ready = if (has_extended2) wait_ready_r.val != 0 else false,
            .listen_timeout_ms = if (has_extended2 and listen_timeout_r.val != 0) listen_timeout_r.val else 8000,
            .shutdown_with_message = if (has_extended2) shutdown_msg_r.val != 0 else false,
            .instances = instances,
        };

        if (instances > 1) {
            // Cluster mode: start N instances
            var first_id: u32 = 0;
            var i: u32 = 0;
            while (i < instances) : (i += 1) {
                const inst_name = std.fmt.allocPrint(self.allocator, "{s}:{d}", .{ name_r.val, i }) catch {
                    try self.sendError(client_fd, request.id, "OutOfMemory");
                    return;
                };
                defer self.allocator.free(inst_name);

                var inst_config = config;
                inst_config.name = inst_name;
                inst_config.instance_id = i;

                const result = self.supervisor.startProcess(inst_config) catch |err| {
                    if (i == 0) {
                        try self.sendError(client_fd, request.id, @errorName(err));
                        return;
                    }
                    continue;
                };

                self.event_loop.addFd(result.stdout_fd, .pipe_read) catch {};
                self.event_loop.addFd(result.stderr_fd, .pipe_read) catch {};

                if (i == 0) first_id = result.id;
            }

            var resp_buf: [4]u8 = undefined;
            _ = protocol.writeU32(&resp_buf, 0, first_id);
            try self.sendResponse(client_fd, request.id, .ok, &resp_buf);
        } else {
            // Fork mode: single instance (existing behavior)
            const result = self.supervisor.startProcess(config) catch |err| {
                try self.sendError(client_fd, request.id, @errorName(err));
                return;
            };

            self.event_loop.addFd(result.stdout_fd, .pipe_read) catch {};
            self.event_loop.addFd(result.stderr_fd, .pipe_read) catch {};

            var resp_buf: [4]u8 = undefined;
            _ = protocol.writeU32(&resp_buf, 0, result.id);
            try self.sendResponse(client_fd, request.id, .ok, &resp_buf);
        }

        // Auto-save state
        self.autoSaveState();
    }

    fn handleProcessStop(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Payload: process_id(u32) + signal(u8) + timeout_ms(u32)
        const data = request.payload;
        var off: usize = 0;

        const id_r = protocol.readU32(data, off);
        off = id_r.next;
        const sig_r = protocol.readU8(data, off);
        off = sig_r.next;
        const timeout_r = protocol.readU32(data, off);

        self.supervisor.stopProcess(id_r.val, sig_r.val, timeout_r.val) catch |err| {
            try self.sendError(client_fd, request.id, @errorName(err));
            return;
        };

        try self.sendResponse(client_fd, request.id, .ok, &[_]u8{});

        // Auto-save state
        self.autoSaveState();
    }

    fn handleProcessDelete(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        const data = request.payload;
        const id_r = protocol.readU32(data, 0);

        self.supervisor.deleteProcess(id_r.val) catch |err| {
            try self.sendError(client_fd, request.id, @errorName(err));
            return;
        };

        try self.sendResponse(client_fd, request.id, .ok, &[_]u8{});
    }

    fn handleProcessList(self: *Self, client_fd: posix.fd_t, req_id: u32) !void {
        const procs = try self.supervisor.listProcesses();
        defer self.supervisor.freeProcessList(procs);

        // Encode: count(u32) + [id(u32) + name(string) + pid(u32) + status(u8) + memory(u64) + uptime(u64) + restarts(u32)]...
        var buf: std.ArrayList(u8) = .{};
        defer buf.deinit(self.allocator);

        // Reserve space and write count
        try buf.appendNTimes(self.allocator, 0, 4);
        std.mem.writeInt(u32, buf.items[0..4], @intCast(procs.len), .little);

        for (procs) |proc| {
            // id
            var tmp: [8]u8 = undefined;
            std.mem.writeInt(u32, tmp[0..4], proc.id, .little);
            try buf.appendSlice(self.allocator, tmp[0..4]);

            // name (length-prefixed)
            std.mem.writeInt(u32, tmp[0..4], @intCast(proc.name.len), .little);
            try buf.appendSlice(self.allocator, tmp[0..4]);
            try buf.appendSlice(self.allocator, proc.name);

            // pid
            std.mem.writeInt(u32, tmp[0..4], @intCast(proc.pid), .little);
            try buf.appendSlice(self.allocator, tmp[0..4]);

            // status
            try buf.append(self.allocator, @intFromEnum(proc.status));

            // memory_bytes
            std.mem.writeInt(u64, tmp[0..8], proc.memory_bytes, .little);
            try buf.appendSlice(self.allocator, tmp[0..8]);

            // uptime_ms
            std.mem.writeInt(u64, tmp[0..8], proc.uptime_ms, .little);
            try buf.appendSlice(self.allocator, tmp[0..8]);

            // restart_count
            std.mem.writeInt(u32, tmp[0..4], proc.restart_count, .little);
            try buf.appendSlice(self.allocator, tmp[0..4]);
        }

        try self.sendResponse(client_fd, req_id, .ok, buf.items);
    }

    fn handleLogRead(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Payload: process_id(u32) + lines(u32)
        const data = request.payload;
        var off: usize = 0;

        const id_r = protocol.readU32(data, off);
        off = id_r.next;
        const lines_r = protocol.readU32(data, off);

        const entries = self.log_collector.readLast(id_r.val, lines_r.val) catch |err| {
            try self.sendError(client_fd, request.id, @errorName(err));
            return;
        };
        defer self.log_collector.freeEntries(entries);

        // Encode: count(u32) + [timestamp(u64) + level(u8) + stream(u8) + message(string)]...
        var buf: std.ArrayList(u8) = .{};
        defer buf.deinit(self.allocator);

        var tmp: [8]u8 = undefined;
        std.mem.writeInt(u32, tmp[0..4], @intCast(entries.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);

        for (entries) |entry| {
            // timestamp_ms
            std.mem.writeInt(u64, tmp[0..8], entry.timestamp_ms, .little);
            try buf.appendSlice(self.allocator, tmp[0..8]);

            // level
            try buf.append(self.allocator, entry.level);

            // stream
            try buf.append(self.allocator, entry.stream);

            // message (length-prefixed)
            std.mem.writeInt(u32, tmp[0..4], @intCast(entry.message.len), .little);
            try buf.appendSlice(self.allocator, tmp[0..4]);
            try buf.appendSlice(self.allocator, entry.message);
        }

        try self.sendResponse(client_fd, request.id, .ok, buf.items);
    }

    fn handleProcessRestart(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Payload: process_id(u32)
        const data = request.payload;
        const id_r = protocol.readU32(data, 0);

        const result = self.supervisor.restartProcess(id_r.val) catch |err| {
            try self.sendError(client_fd, request.id, @errorName(err));
            return;
        };

        // Register new pipe fds in event loop
        self.event_loop.addFd(result.stdout_fd, .pipe_read) catch {};
        self.event_loop.addFd(result.stderr_fd, .pipe_read) catch {};

        // Also drain any pending pipe fds from autorestart
        const pending_fds = self.supervisor.drainPendingPipeFds();
        defer self.allocator.free(pending_fds);
        for (pending_fds) |fd| {
            self.event_loop.addFd(fd, .pipe_read) catch {};
        }

        try self.sendResponse(client_fd, request.id, .ok, &[_]u8{});
    }

    fn handleProcessInfo(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Payload: process_id(u32)
        const data = request.payload;
        const id_r = protocol.readU32(data, 0);

        const proc = self.supervisor.getProcess(id_r.val) orelse {
            try self.sendError(client_fd, request.id, "ProcessNotFound");
            return;
        };

        // Update uptime if running
        if (proc.status == .running) {
            const now: u64 = @intCast(std.time.milliTimestamp());
            proc.uptime_ms = now - proc.start_time_ms;
        }

        // Encode full process details:
        // id(u32) + name(string) + pid(u32) + status(u8)
        // + memory(u64) + uptime(u64) + restarts(u32)
        // + consecutive_crashes(u32) + last_restart_ms(u64)
        // + config: script(string) + cwd(string) + interpreter(string)
        // + kill_timeout(u32) + autorestart(u8) + max_restarts(i32)
        // + min_uptime_ms(u64) + restart_delay_ms(u32) + exp_backoff(u8)
        var buf: std.ArrayList(u8) = .{};
        defer buf.deinit(self.allocator);

        var tmp: [8]u8 = undefined;

        // id
        std.mem.writeInt(u32, tmp[0..4], proc.id, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // name
        std.mem.writeInt(u32, tmp[0..4], @intCast(proc.name.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        try buf.appendSlice(self.allocator, proc.name);
        // pid
        std.mem.writeInt(u32, tmp[0..4], @intCast(proc.pid), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // status
        try buf.append(self.allocator, @intFromEnum(proc.status));
        // memory_bytes
        std.mem.writeInt(u64, tmp[0..8], proc.memory_bytes, .little);
        try buf.appendSlice(self.allocator, tmp[0..8]);
        // uptime_ms
        std.mem.writeInt(u64, tmp[0..8], proc.uptime_ms, .little);
        try buf.appendSlice(self.allocator, tmp[0..8]);
        // restart_count
        std.mem.writeInt(u32, tmp[0..4], proc.restart_count, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // consecutive_crashes
        std.mem.writeInt(u32, tmp[0..4], proc.consecutive_crashes, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // last_restart_ms
        std.mem.writeInt(u64, tmp[0..8], proc.last_restart_ms, .little);
        try buf.appendSlice(self.allocator, tmp[0..8]);
        // config: script
        std.mem.writeInt(u32, tmp[0..4], @intCast(proc.config.script.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        try buf.appendSlice(self.allocator, proc.config.script);
        // config: cwd
        std.mem.writeInt(u32, tmp[0..4], @intCast(proc.config.cwd.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        try buf.appendSlice(self.allocator, proc.config.cwd);
        // config: interpreter
        const interp = proc.config.interpreter orelse "";
        std.mem.writeInt(u32, tmp[0..4], @intCast(interp.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        if (interp.len > 0) try buf.appendSlice(self.allocator, interp);
        // config: kill_timeout_ms
        std.mem.writeInt(u32, tmp[0..4], proc.config.kill_timeout_ms, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // config: autorestart
        try buf.append(self.allocator, if (proc.config.autorestart) @as(u8, 1) else 0);
        // config: max_restarts
        std.mem.writeInt(i32, tmp[0..4], proc.config.max_restarts, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // config: min_uptime_ms
        std.mem.writeInt(u64, tmp[0..8], proc.config.min_uptime_ms, .little);
        try buf.appendSlice(self.allocator, tmp[0..8]);
        // config: restart_delay_ms
        std.mem.writeInt(u32, tmp[0..4], proc.config.restart_delay_ms, .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        // config: exp_backoff
        try buf.append(self.allocator, if (proc.config.exp_backoff) @as(u8, 1) else 0);

        // Extended fields (batch 2)
        // max_memory_restart
        std.mem.writeInt(u64, tmp[0..8], proc.config.max_memory_restart, .little);
        try buf.appendSlice(self.allocator, tmp[0..8]);
        // watch
        try buf.append(self.allocator, if (proc.config.watch) @as(u8, 1) else 0);
        // cron_restart
        const cron = proc.config.cron_restart orelse "";
        std.mem.writeInt(u32, tmp[0..4], @intCast(cron.len), .little);
        try buf.appendSlice(self.allocator, tmp[0..4]);
        if (cron.len > 0) try buf.appendSlice(self.allocator, cron);
        // wait_ready
        try buf.append(self.allocator, if (proc.config.wait_ready) @as(u8, 1) else 0);
        // shutdown_with_message
        try buf.append(self.allocator, if (proc.config.shutdown_with_message) @as(u8, 1) else 0);

        try self.sendResponse(client_fd, request.id, .ok, buf.items);
    }

    fn handleProcessScale(self: *Self, client_fd: posix.fd_t, request: protocol.Request) !void {
        // Payload: name(string) + target_count(u32)
        const data = request.payload;
        var off: usize = 0;

        const name_r = protocol.readString(data, off);
        off = name_r.next;
        const count_r = protocol.readU32(data, off);

        if (name_r.val.len == 0) {
            try self.sendError(client_fd, request.id, "empty process name");
            return;
        }

        const result = self.supervisor.scaleCluster(name_r.val, count_r.val) catch |err| {
            try self.sendError(client_fd, request.id, @errorName(err));
            return;
        };

        // Register any new pipe fds from scaling up
        const pending_fds = self.supervisor.drainPendingPipeFds();
        defer self.allocator.free(pending_fds);
        for (pending_fds) |fd| {
            self.event_loop.addFd(fd, .pipe_read) catch {};
        }

        // Return started(u32) + stopped(u32)
        var resp_buf: [8]u8 = undefined;
        _ = protocol.writeU32(&resp_buf, 0, result.started);
        _ = protocol.writeU32(&resp_buf, 4, result.stopped);
        try self.sendResponse(client_fd, request.id, .ok, &resp_buf);

        self.autoSaveState();
    }

    fn handleStateSave(self: *Self, client_fd: posix.fd_t, req_id: u32) !void {
        const p = self.persistence orelse {
            try self.sendError(client_fd, req_id, "persistence not configured");
            return;
        };

        const procs = self.supervisor.getAllConfigs() catch |err| {
            try self.sendError(client_fd, req_id, @errorName(err));
            return;
        };
        defer self.allocator.free(procs);

        p.saveState(procs) catch |err| {
            try self.sendError(client_fd, req_id, @errorName(err));
            return;
        };

        try self.sendResponse(client_fd, req_id, .ok, "state saved");
    }

    fn handleStateLoad(self: *Self, client_fd: posix.fd_t, req_id: u32) !void {
        const p = self.persistence orelse {
            try self.sendError(client_fd, req_id, "persistence not configured");
            return;
        };

        const configs = p.loadState() catch |err| {
            try self.sendError(client_fd, req_id, @errorName(err));
            return;
        };
        defer {
            for (configs) |cfg| {
                self.allocator.free(cfg.name);
                self.allocator.free(cfg.script);
                self.allocator.free(cfg.cwd);
                if (cfg.interpreter) |interp| self.allocator.free(interp);
                if (cfg.watch_paths) |wp| self.allocator.free(wp);
                if (cfg.watch_ignore) |wi| self.allocator.free(wi);
                if (cfg.cron_restart) |cr| self.allocator.free(cr);
            }
            self.allocator.free(configs);
        }

        var started: u32 = 0;
        for (configs) |cfg| {
            const result = self.supervisor.startProcess(cfg) catch continue;
            self.event_loop.addFd(result.stdout_fd, .pipe_read) catch {};
            self.event_loop.addFd(result.stderr_fd, .pipe_read) catch {};
            started += 1;
        }

        // Return number of processes started
        var resp_buf: [4]u8 = undefined;
        _ = protocol.writeU32(&resp_buf, 0, started);
        try self.sendResponse(client_fd, req_id, .ok, &resp_buf);
    }

    fn handleShutdown(self: *Self, client_fd: posix.fd_t, req_id: u32) !void {
        try self.sendResponse(client_fd, req_id, .ok, "shutting down");
        self.shutdown_requested = true;
    }

    fn sendResponse(self: *Self, client_fd: posix.fd_t, req_id: u32, status: protocol.Status, payload: []const u8) !void {
        const resp = protocol.Response{
            .id = req_id,
            .status = status,
            .payload = payload,
        };
        const resp_data = try protocol.encodeResponse(self.allocator, resp);
        defer self.allocator.free(resp_data);

        const msg = try protocol.buildMessage(self.allocator, resp_data);
        defer self.allocator.free(msg);

        _ = posix.write(client_fd, msg) catch {};
    }

    fn sendError(self: *Self, client_fd: posix.fd_t, req_id: u32, err_msg: []const u8) !void {
        try self.sendResponse(client_fd, req_id, .err, err_msg);
    }

    /// Remove and clean up a client connection
    pub fn removeClient(self: *Self, fd: posix.fd_t) void {
        self.event_loop.removeFd(fd);
        if (self.clients.get(fd)) |client| {
            client.recv_buf.deinit(self.allocator);
            self.allocator.destroy(client);
        }
        _ = self.clients.remove(fd);
        posix.close(fd);
    }

    fn autoSaveState(self: *Self) void {
        const p = self.persistence orelse return;
        const procs = self.supervisor.getAllConfigs() catch return;
        defer self.allocator.free(procs);
        p.saveState(procs) catch {};
    }

    pub fn setPersistence(self: *Self, p: *Persistence) void {
        self.persistence = p;
    }

    pub fn isShutdownRequested(self: *Self) bool {
        return self.shutdown_requested;
    }
};
