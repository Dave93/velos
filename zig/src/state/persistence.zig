const std = @import("std");
const posix = std.posix;

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
};
