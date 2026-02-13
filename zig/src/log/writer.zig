const std = @import("std");

/// Writes log lines to files on disk with optional rotation.
/// Each process has two log files: <name>-out.log (stdout) and <name>-err.log (stderr)
pub const LogWriter = struct {
    const Self = @This();

    log_dir: []const u8,
    allocator: std.mem.Allocator,

    // Per-file size tracking (key: "name-out" or "name-err")
    file_sizes: std.StringHashMap(u64),

    pub fn init(allocator: std.mem.Allocator, log_dir: []const u8) Self {
        return Self{
            .log_dir = log_dir,
            .allocator = allocator,
            .file_sizes = std.StringHashMap(u64).init(allocator),
        };
    }

    pub fn deinit(self: *Self) void {
        var it = self.file_sizes.keyIterator();
        while (it.next()) |key| {
            self.allocator.free(key.*);
        }
        self.file_sizes.deinit();
    }

    /// Append a line to the appropriate log file.
    /// stream: 0 = stdout (-out.log), 1 = stderr (-err.log)
    /// Performs rotation if max_size is exceeded.
    pub fn writeLine(self: *Self, name: []const u8, stream: u8, line: []const u8) !void {
        self.writeLineWithRotation(name, stream, line, 10 * 1024 * 1024, 30) catch {};
    }

    /// Write line with explicit rotation parameters.
    pub fn writeLineWithRotation(
        self: *Self,
        name: []const u8,
        stream: u8,
        line: []const u8,
        max_size: u64,
        retain_count: u32,
    ) !void {
        const suffix = if (stream == 0) "-out.log" else "-err.log";
        const path = try std.fmt.allocPrint(self.allocator, "{s}/{s}{s}", .{ self.log_dir, name, suffix });
        defer self.allocator.free(path);

        // Check if rotation is needed
        const size_key_suffix = if (stream == 0) "-out" else "-err";
        const size_key = try std.fmt.allocPrint(self.allocator, "{s}{s}", .{ name, size_key_suffix });

        var current_size = self.file_sizes.get(size_key) orelse blk: {
            // Get current file size if it exists
            const file = std.fs.openFileAbsolute(path, .{}) catch {
                break :blk @as(u64, 0);
            };
            defer file.close();
            const stat = file.stat() catch break :blk @as(u64, 0);
            break :blk stat.size;
        };

        // Check if rotation needed
        if (max_size > 0 and current_size >= max_size) {
            self.rotateFile(path, retain_count) catch {};
            current_size = 0;
        }

        const file = try std.fs.createFileAbsolute(path, .{
            .truncate = false,
        });
        defer file.close();

        // Seek to end for append
        try file.seekFromEnd(0);
        try file.writeAll(line);
        const line_len: u64 = line.len;
        var total_written = line_len;
        // Ensure newline
        if (line.len == 0 or line[line.len - 1] != '\n') {
            try file.writeAll("\n");
            total_written += 1;
        }

        current_size += total_written;

        // Update tracked size
        if (self.file_sizes.getPtr(size_key)) |ptr| {
            ptr.* = current_size;
            self.allocator.free(size_key);
        } else {
            self.file_sizes.put(size_key, current_size) catch {
                self.allocator.free(size_key);
            };
        }
    }

    /// Rotate log files: current → .1, .1 → .2, etc. Keep max retain_count.
    fn rotateFile(self: *Self, path: []const u8, retain_count: u32) !void {
        // Delete oldest if it exists
        if (retain_count > 0) {
            const oldest = try std.fmt.allocPrint(self.allocator, "{s}.{d}", .{ path, retain_count });
            defer self.allocator.free(oldest);
            std.fs.deleteFileAbsolute(oldest) catch {};
        }

        // Shift .N-1 → .N, .N-2 → .N-1, etc.
        var i: u32 = retain_count;
        while (i > 1) : (i -= 1) {
            const from = std.fmt.allocPrint(self.allocator, "{s}.{d}", .{ path, i - 1 }) catch continue;
            defer self.allocator.free(from);
            const to = std.fmt.allocPrint(self.allocator, "{s}.{d}", .{ path, i }) catch continue;
            defer self.allocator.free(to);
            std.fs.renameAbsolute(from, to) catch {};
        }

        // Rename current → .1
        if (retain_count > 0) {
            const first = try std.fmt.allocPrint(self.allocator, "{s}.1", .{path});
            defer self.allocator.free(first);
            std.fs.renameAbsolute(path, first) catch {};
        }
    }

    /// Delete log files for a process
    pub fn deleteFiles(self: *Self, name: []const u8) void {
        const out_path = std.fmt.allocPrint(self.allocator, "{s}/{s}-out.log", .{ self.log_dir, name }) catch return;
        defer self.allocator.free(out_path);
        std.fs.deleteFileAbsolute(out_path) catch {};

        const err_path = std.fmt.allocPrint(self.allocator, "{s}/{s}-err.log", .{ self.log_dir, name }) catch return;
        defer self.allocator.free(err_path);
        std.fs.deleteFileAbsolute(err_path) catch {};
    }
};
