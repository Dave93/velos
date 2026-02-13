const std = @import("std");
const posix = std.posix;
const ProcessInfo = @import("../process/supervisor.zig").ProcessInfo;
const ProcessConfig = @import("../process/supervisor.zig").ProcessConfig;

/// Manages the ~/.velos/ runtime directory, PID file, and state persistence.
pub const Persistence = struct {
    const Self = @This();

    state_dir: []const u8, // e.g. "/Users/foo/.velos"
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, state_dir: []const u8) Self {
        return Self{
            .state_dir = state_dir,
            .allocator = allocator,
        };
    }

    /// Create the state directory and subdirectories if they don't exist.
    pub fn ensureDirectories(self: *Self) !void {
        // Create state_dir
        makeDirIfNotExists(self.state_dir) catch |err| {
            if (err != error.PathAlreadyExists) return err;
        };

        // Create logs subdirectory
        const logs_dir = try std.fmt.allocPrint(self.allocator, "{s}/logs", .{self.state_dir});
        defer self.allocator.free(logs_dir);
        makeDirIfNotExists(logs_dir) catch |err| {
            if (err != error.PathAlreadyExists) return err;
        };
    }

    fn makeDirIfNotExists(path: []const u8) !void {
        std.fs.makeDirAbsolute(path) catch |err| {
            if (err == error.PathAlreadyExists) return;
            return err;
        };
    }

    /// Write the daemon PID to ~/.velos/velos.pid
    pub fn writePidFile(self: *Self) !void {
        const pid_path = try std.fmt.allocPrint(self.allocator, "{s}/velos.pid", .{self.state_dir});
        defer self.allocator.free(pid_path);

        const pid = std.c.getpid();
        var buf: [20]u8 = undefined;
        const pid_str = std.fmt.bufPrint(&buf, "{d}", .{pid}) catch unreachable;

        const file = try std.fs.createFileAbsolute(pid_path, .{});
        defer file.close();
        try file.writeAll(pid_str);
    }

    /// Remove the PID file
    pub fn removePidFile(self: *Self) void {
        const pid_path = std.fmt.allocPrint(self.allocator, "{s}/velos.pid", .{self.state_dir}) catch return;
        defer self.allocator.free(pid_path);
        std.fs.deleteFileAbsolute(pid_path) catch {};
    }

    /// Read the daemon PID from the PID file. Returns null if file doesn't exist.
    pub fn readPidFile(self: *Self) !?posix.pid_t {
        const pid_path = try std.fmt.allocPrint(self.allocator, "{s}/velos.pid", .{self.state_dir});
        defer self.allocator.free(pid_path);

        const file = std.fs.openFileAbsolute(pid_path, .{}) catch |err| {
            if (err == error.FileNotFound) return null;
            return err;
        };
        defer file.close();

        var buf: [20]u8 = undefined;
        const n = try file.readAll(&buf);
        if (n == 0) return null;

        const pid_str = std.mem.trimRight(u8, buf[0..n], &[_]u8{ '\n', '\r', ' ' });
        const pid = std.fmt.parseInt(posix.pid_t, pid_str, 10) catch return null;
        return pid;
    }

    /// Get the socket path
    pub fn socketPath(self: *Self) ![]u8 {
        return try std.fmt.allocPrint(self.allocator, "{s}/velos.sock", .{self.state_dir});
    }

    /// Get the log directory path
    pub fn logDir(self: *Self) ![]u8 {
        return try std.fmt.allocPrint(self.allocator, "{s}/logs", .{self.state_dir});
    }

    /// Save process configs to state.json using a simple binary format.
    /// Format: count(u32) + [name_len(u32) + name + script_len(u32) + script + cwd_len(u32) + cwd
    ///         + interp_len(u32) + interp + kill_timeout(u32) + autorestart(u8)
    ///         + max_restarts(i32) + min_uptime_ms(u64) + restart_delay_ms(u32) + exp_backoff(u8)]...
    pub fn saveState(self: *Self, procs: []*ProcessInfo) !void {
        const path = try std.fmt.allocPrint(self.allocator, "{s}/state.bin", .{self.state_dir});
        defer self.allocator.free(path);

        const file = try std.fs.createFileAbsolute(path, .{});
        defer file.close();

        // Write count
        var tmp: [8]u8 = undefined;
        std.mem.writeInt(u32, tmp[0..4], @intCast(procs.len), .little);
        try file.writeAll(tmp[0..4]);

        for (procs) |proc| {
            const cfg = proc.config;

            // name
            std.mem.writeInt(u32, tmp[0..4], @intCast(cfg.name.len), .little);
            try file.writeAll(tmp[0..4]);
            try file.writeAll(cfg.name);

            // script
            std.mem.writeInt(u32, tmp[0..4], @intCast(cfg.script.len), .little);
            try file.writeAll(tmp[0..4]);
            try file.writeAll(cfg.script);

            // cwd
            std.mem.writeInt(u32, tmp[0..4], @intCast(cfg.cwd.len), .little);
            try file.writeAll(tmp[0..4]);
            try file.writeAll(cfg.cwd);

            // interpreter
            const interp = cfg.interpreter orelse "";
            std.mem.writeInt(u32, tmp[0..4], @intCast(interp.len), .little);
            try file.writeAll(tmp[0..4]);
            if (interp.len > 0) try file.writeAll(interp);

            // kill_timeout_ms
            std.mem.writeInt(u32, tmp[0..4], cfg.kill_timeout_ms, .little);
            try file.writeAll(tmp[0..4]);

            // autorestart
            tmp[0] = if (cfg.autorestart) 1 else 0;
            try file.writeAll(tmp[0..1]);

            // max_restarts
            std.mem.writeInt(i32, tmp[0..4], cfg.max_restarts, .little);
            try file.writeAll(tmp[0..4]);

            // min_uptime_ms
            std.mem.writeInt(u64, tmp[0..8], cfg.min_uptime_ms, .little);
            try file.writeAll(tmp[0..8]);

            // restart_delay_ms
            std.mem.writeInt(u32, tmp[0..4], cfg.restart_delay_ms, .little);
            try file.writeAll(tmp[0..4]);

            // exp_backoff
            tmp[0] = if (cfg.exp_backoff) 1 else 0;
            try file.writeAll(tmp[0..1]);

            // max_memory_restart
            std.mem.writeInt(u64, tmp[0..8], cfg.max_memory_restart, .little);
            try file.writeAll(tmp[0..8]);

            // watch
            tmp[0] = if (cfg.watch) 1 else 0;
            try file.writeAll(tmp[0..1]);

            // watch_delay_ms
            std.mem.writeInt(u32, tmp[0..4], cfg.watch_delay_ms, .little);
            try file.writeAll(tmp[0..4]);

            // watch_paths
            const wp = cfg.watch_paths orelse "";
            std.mem.writeInt(u32, tmp[0..4], @intCast(wp.len), .little);
            try file.writeAll(tmp[0..4]);
            if (wp.len > 0) try file.writeAll(wp);

            // watch_ignore
            const wi = cfg.watch_ignore orelse "";
            std.mem.writeInt(u32, tmp[0..4], @intCast(wi.len), .little);
            try file.writeAll(tmp[0..4]);
            if (wi.len > 0) try file.writeAll(wi);

            // cron_restart
            const cron = cfg.cron_restart orelse "";
            std.mem.writeInt(u32, tmp[0..4], @intCast(cron.len), .little);
            try file.writeAll(tmp[0..4]);
            if (cron.len > 0) try file.writeAll(cron);

            // wait_ready
            tmp[0] = if (cfg.wait_ready) 1 else 0;
            try file.writeAll(tmp[0..1]);

            // listen_timeout_ms
            std.mem.writeInt(u32, tmp[0..4], cfg.listen_timeout_ms, .little);
            try file.writeAll(tmp[0..4]);

            // shutdown_with_message
            tmp[0] = if (cfg.shutdown_with_message) 1 else 0;
            try file.writeAll(tmp[0..1]);

            // instances
            std.mem.writeInt(u32, tmp[0..4], cfg.instances, .little);
            try file.writeAll(tmp[0..4]);

            // instance_id
            std.mem.writeInt(u32, tmp[0..4], cfg.instance_id, .little);
            try file.writeAll(tmp[0..4]);
        }
    }

    /// Load process configs from state.bin. Caller must free each config's strings and the slice.
    pub fn loadState(self: *Self) ![]ProcessConfig {
        const path = try std.fmt.allocPrint(self.allocator, "{s}/state.bin", .{self.state_dir});
        defer self.allocator.free(path);

        const file = std.fs.openFileAbsolute(path, .{}) catch |err| {
            if (err == error.FileNotFound) return try self.allocator.alloc(ProcessConfig, 0);
            return err;
        };
        defer file.close();

        const stat = try file.stat();
        if (stat.size < 4) return try self.allocator.alloc(ProcessConfig, 0);

        const data = try self.allocator.alloc(u8, stat.size);
        defer self.allocator.free(data);
        const read_len = try file.readAll(data);
        if (read_len < 4) return try self.allocator.alloc(ProcessConfig, 0);

        var off: usize = 0;
        const count = std.mem.readInt(u32, data[0..4], .little);
        off = 4;

        var configs: std.ArrayList(ProcessConfig) = .{};

        var i: u32 = 0;
        while (i < count) : (i += 1) {
            const cfg = self.readOneConfig(data, &off) catch break;
            configs.append(self.allocator, cfg) catch break;
        }

        return configs.toOwnedSlice(self.allocator);
    }

    fn readOneConfig(self: *Self, data: []const u8, off: *usize) !ProcessConfig {
        const name = try self.readBinString(data, off);
        const script = try self.readBinString(data, off);
        const cwd = try self.readBinString(data, off);
        const interp_str = try self.readBinString(data, off);
        const interpreter: ?[]const u8 = if (interp_str.len == 0) blk: {
            self.allocator.free(interp_str);
            break :blk null;
        } else interp_str;

        if (off.* + 4 > data.len) return error.TruncatedState;
        const kill_timeout = std.mem.readInt(u32, data[off.*..][0..4], .little);
        off.* += 4;

        if (off.* >= data.len) return error.TruncatedState;
        const autorestart = data[off.*] != 0;
        off.* += 1;

        if (off.* + 4 > data.len) return error.TruncatedState;
        const max_restarts = std.mem.readInt(i32, data[off.*..][0..4], .little);
        off.* += 4;

        if (off.* + 8 > data.len) return error.TruncatedState;
        const min_uptime_ms = std.mem.readInt(u64, data[off.*..][0..8], .little);
        off.* += 8;

        if (off.* + 4 > data.len) return error.TruncatedState;
        const restart_delay_ms = std.mem.readInt(u32, data[off.*..][0..4], .little);
        off.* += 4;

        if (off.* >= data.len) return error.TruncatedState;
        const exp_backoff = data[off.*] != 0;
        off.* += 1;

        // Extended fields (batch 2) — optional for backward compat with old state files
        var max_memory_restart: u64 = 0;
        var watch: bool = false;
        var watch_delay_ms: u32 = 1000;
        var watch_paths: ?[]const u8 = null;
        var watch_ignore: ?[]const u8 = null;
        var cron_restart: ?[]const u8 = null;
        var wait_ready: bool = false;
        var listen_timeout_ms: u32 = 8000;
        var shutdown_with_message: bool = false;

        if (off.* + 8 <= data.len) {
            max_memory_restart = std.mem.readInt(u64, data[off.*..][0..8], .little);
            off.* += 8;

            if (off.* < data.len) {
                watch = data[off.*] != 0;
                off.* += 1;
            }

            if (off.* + 4 <= data.len) {
                watch_delay_ms = std.mem.readInt(u32, data[off.*..][0..4], .little);
                off.* += 4;
            }

            // watch_paths
            if (off.* + 4 <= data.len) {
                const wp_str = try self.readBinString(data, off);
                watch_paths = if (wp_str.len == 0) blk: {
                    self.allocator.free(wp_str);
                    break :blk null;
                } else wp_str;
            }

            // watch_ignore
            if (off.* + 4 <= data.len) {
                const wi_str = try self.readBinString(data, off);
                watch_ignore = if (wi_str.len == 0) blk: {
                    self.allocator.free(wi_str);
                    break :blk null;
                } else wi_str;
            }

            // cron_restart
            if (off.* + 4 <= data.len) {
                const cron_str = try self.readBinString(data, off);
                cron_restart = if (cron_str.len == 0) blk: {
                    self.allocator.free(cron_str);
                    break :blk null;
                } else cron_str;
            }

            if (off.* < data.len) {
                wait_ready = data[off.*] != 0;
                off.* += 1;
            }

            if (off.* + 4 <= data.len) {
                listen_timeout_ms = std.mem.readInt(u32, data[off.*..][0..4], .little);
                off.* += 4;
            }

            if (off.* < data.len) {
                shutdown_with_message = data[off.*] != 0;
                off.* += 1;
            }
        }

        // Phase 6 extended fields: instances + instance_id
        var instances: u32 = 1;
        var instance_id: u32 = 0;
        if (off.* + 4 <= data.len) {
            instances = std.mem.readInt(u32, data[off.*..][0..4], .little);
            off.* += 4;
        }
        if (off.* + 4 <= data.len) {
            instance_id = std.mem.readInt(u32, data[off.*..][0..4], .little);
            off.* += 4;
        }

        return ProcessConfig{
            .name = name,
            .script = script,
            .cwd = cwd,
            .interpreter = interpreter,
            .kill_timeout_ms = kill_timeout,
            .autorestart = autorestart,
            .max_restarts = max_restarts,
            .min_uptime_ms = min_uptime_ms,
            .restart_delay_ms = restart_delay_ms,
            .exp_backoff = exp_backoff,
            .max_memory_restart = max_memory_restart,
            .watch = watch,
            .watch_delay_ms = watch_delay_ms,
            .watch_paths = watch_paths,
            .watch_ignore = watch_ignore,
            .cron_restart = cron_restart,
            .wait_ready = wait_ready,
            .listen_timeout_ms = listen_timeout_ms,
            .shutdown_with_message = shutdown_with_message,
            .instances = instances,
            .instance_id = instance_id,
        };
    }

    fn readBinString(self: *Self, data: []const u8, off: *usize) ![]u8 {
        if (off.* + 4 > data.len) return error.TruncatedState;
        const len = std.mem.readInt(u32, data[off.*..][0..4], .little);
        off.* += 4;
        if (off.* + len > data.len) return error.TruncatedState;
        const str = try self.allocator.dupe(u8, data[off.*..][0..len]);
        off.* += len;
        return str;
    }
};
