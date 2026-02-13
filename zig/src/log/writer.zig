const std = @import("std");

/// Writes log lines to files on disk.
/// Each process has two log files: <name>-out.log (stdout) and <name>-err.log (stderr)
pub const LogWriter = struct {
    const Self = @This();

    log_dir: []const u8,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, log_dir: []const u8) Self {
        return Self{
            .log_dir = log_dir,
            .allocator = allocator,
        };
    }

    /// Append a line to the appropriate log file.
    /// stream: 0 = stdout (-out.log), 1 = stderr (-err.log)
    pub fn writeLine(self: *Self, name: []const u8, stream: u8, line: []const u8) !void {
        const suffix = if (stream == 0) "-out.log" else "-err.log";
        const path = try std.fmt.allocPrint(self.allocator, "{s}/{s}{s}", .{ self.log_dir, name, suffix });
        defer self.allocator.free(path);

        const file = try std.fs.createFileAbsolute(path, .{
            .truncate = false,
        });
        defer file.close();

        // Seek to end for append
        try file.seekFromEnd(0);
        try file.writeAll(line);
        // Ensure newline
        if (line.len == 0 or line[line.len - 1] != '\n') {
            try file.writeAll("\n");
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
