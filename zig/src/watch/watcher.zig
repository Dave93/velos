const std = @import("std");
const posix = std.posix;
const builtin = @import("builtin");

/// File watcher for detecting changes in watched directories.
/// Uses kqueue (EVFILT_VNODE) on macOS, inotify on Linux.
pub const Watcher = struct {
    const Self = @This();

    allocator: std.mem.Allocator,

    /// Dedicated kqueue/inotify fd for file watching
    watch_fd: posix.fd_t,

    /// Open directory fds being watched (macOS kqueue needs open fds)
    watched_fds: std.ArrayList(posix.fd_t),

    /// Semicolon-separated watch paths (owned)
    watch_paths: ?[]const u8,

    /// Semicolon-separated ignore patterns (owned)
    ignore_patterns: ?[]const u8,

    /// Debounce delay in milliseconds
    delay_ms: u32,

    /// Timestamp (ms) of last detected change
    last_change_ms: u64,

    /// Whether the watcher is active
    enabled: bool,

    pub fn init(allocator: std.mem.Allocator, paths: ?[]const u8, ignore: ?[]const u8, delay_ms: u32) Self {
        // Create platform-specific watch fd
        const wfd = createWatchFd() catch -1;

        return Self{
            .allocator = allocator,
            .watch_fd = wfd,
            .watched_fds = .{},
            .watch_paths = if (paths) |p| allocator.dupe(u8, p) catch null else null,
            .ignore_patterns = if (ignore) |i| allocator.dupe(u8, i) catch null else null,
            .delay_ms = if (delay_ms == 0) 1000 else delay_ms,
            .last_change_ms = 0,
            .enabled = wfd != -1,
        };
    }

    pub fn deinit(self: *Self) void {
        // Close all watched directory fds
        for (self.watched_fds.items) |fd| {
            posix.close(fd);
        }
        self.watched_fds.deinit(self.allocator);

        // Close the watch fd
        if (self.watch_fd != -1) {
            posix.close(self.watch_fd);
        }

        if (self.watch_paths) |p| self.allocator.free(p);
        if (self.ignore_patterns) |i| self.allocator.free(i);
    }

    /// Add directories to watch. Call after init.
    /// `cwd` is used as default watch path if no explicit paths were set.
    pub fn setup(self: *Self, cwd: []const u8) !void {
        if (!self.enabled) return;

        const paths = self.watch_paths orelse cwd;

        // Parse semicolon-separated paths
        var iter = std.mem.splitScalar(u8, paths, ';');
        while (iter.next()) |raw_path| {
            const path = std.mem.trim(u8, raw_path, " \t");
            if (path.len == 0) continue;
            if (self.shouldIgnore(path)) continue;

            self.addWatchPath(path, cwd) catch {
                // Skip paths we can't watch (e.g., doesn't exist)
                continue;
            };
        }
    }

    /// Check if any watched files changed. Non-blocking.
    /// Returns true if a change was detected and debounce period elapsed.
    pub fn checkForChanges(self: *Self) bool {
        if (!self.enabled or self.watch_fd == -1) return false;

        if (comptime builtin.os.tag == .macos) {
            return self.checkKqueue();
        } else if (comptime builtin.os.tag == .linux) {
            return self.checkInotify();
        }
        return false;
    }

    // ---- macOS kqueue implementation ----

    fn checkKqueue(self: *Self) bool {
        if (comptime builtin.os.tag != .macos) return false;

        var events: [16]std.c.Kevent = undefined;
        const timeout = std.c.timespec{ .sec = 0, .nsec = 0 }; // non-blocking
        const empty_changelist: [0]std.c.Kevent = .{};

        const n = std.c.kevent(
            self.watch_fd,
            &empty_changelist, // no changes to register
            0,
            &events,
            @intCast(events.len),
            &timeout,
        );

        if (n <= 0) {
            // No events — check if debounce period has passed since last change
            return self.checkDebounce();
        }

        // Got change events — update timestamp
        const now: u64 = @intCast(std.time.milliTimestamp());
        self.last_change_ms = now;

        // Re-register the events (kqueue EVFILT_VNODE is one-shot by default with EV_CLEAR)
        // With EV_CLEAR they auto-reset, so no re-registration needed.

        return self.checkDebounce();
    }

    fn addWatchPathKqueue(self: *Self, fd: posix.fd_t) !void {
        if (comptime builtin.os.tag != .macos) return;

        var changelist: [1]std.c.Kevent = .{std.c.Kevent{
            .ident = @intCast(fd),
            .filter = std.c.EVFILT.VNODE,
            .flags = std.c.EV.ADD | std.c.EV.CLEAR,
            .fflags = std.c.NOTE.WRITE | std.c.NOTE.DELETE | std.c.NOTE.RENAME | std.c.NOTE.ATTRIB,
            .data = 0,
            .udata = 0,
        }};

        const empty_events: [0]std.c.Kevent = .{};
        const ret = std.c.kevent(
            self.watch_fd,
            &changelist,
            1,
            @constCast(&empty_events),
            0,
            null, // no timeout
        );

        if (ret < 0) return error.KqueueRegisterFailed;
    }

    // ---- Linux inotify implementation ----

    fn checkInotify(self: *Self) bool {
        if (comptime builtin.os.tag != .linux) return false;

        // Read inotify events (non-blocking)
        var buf: [4096]u8 = undefined;
        const n = posix.read(self.watch_fd, &buf) catch {
            return self.checkDebounce();
        };

        if (n > 0) {
            const now: u64 = @intCast(std.time.milliTimestamp());
            self.last_change_ms = now;
        }

        return self.checkDebounce();
    }

    // ---- Shared helpers ----

    fn addWatchPath(self: *Self, path: []const u8, cwd: []const u8) !void {
        // Resolve absolute path
        var abs_buf: [std.fs.max_path_bytes]u8 = undefined;
        const abs_path = resolvePath(path, cwd, &abs_buf) orelse return error.InvalidPath;
        const abs_z = self.allocator.dupeZ(u8, abs_path) catch return error.OutOfMemory;
        defer self.allocator.free(abs_z);

        if (comptime builtin.os.tag == .macos) {
            // Open directory with O_EVTONLY (for kqueue watching without preventing unmount)
            const fd = posix.openatZ(posix.AT.FDCWD, abs_z, .{ .ACCMODE = .RDONLY }, 0) catch
                return error.OpenFailed;
            try self.addWatchPathKqueue(fd);
            try self.watched_fds.append(self.allocator, fd);
        } else if (comptime builtin.os.tag == .linux) {
            // inotify_add_watch
            const wd = std.os.linux.inotify_add_watch(
                self.watch_fd,
                abs_z,
                std.os.linux.IN.MODIFY | std.os.linux.IN.CREATE | std.os.linux.IN.DELETE | std.os.linux.IN.MOVE_SELF,
            );
            if (wd < 0) return error.InotifyAddFailed;
        }
    }

    fn checkDebounce(self: *Self) bool {
        if (self.last_change_ms == 0) return false;

        const now: u64 = @intCast(std.time.milliTimestamp());
        if (now - self.last_change_ms >= self.delay_ms) {
            self.last_change_ms = 0; // Reset after reporting
            return true;
        }
        return false;
    }

    /// Check if a path matches any ignore pattern (simple substring match).
    fn shouldIgnore(self: *Self, path: []const u8) bool {
        const patterns = self.ignore_patterns orelse return false;

        var iter = std.mem.splitScalar(u8, patterns, ';');
        while (iter.next()) |raw_pat| {
            const pat = std.mem.trim(u8, raw_pat, " \t");
            if (pat.len == 0) continue;
            if (std.mem.indexOf(u8, path, pat) != null) return true;
        }
        return false;
    }

    /// Resolve a path relative to cwd, writing the result into `buf`.
    fn resolvePath(path: []const u8, cwd: []const u8, buf: *[std.fs.max_path_bytes]u8) ?[]const u8 {
        if (path.len > 0 and path[0] == '/') {
            // Already absolute
            return path;
        }
        // Relative: prepend cwd
        if (cwd.len + 1 + path.len > buf.len) return null;
        @memcpy(buf[0..cwd.len], cwd);
        buf[cwd.len] = '/';
        @memcpy(buf[cwd.len + 1 ..][0..path.len], path);
        return buf[0 .. cwd.len + 1 + path.len];
    }

    /// Create platform watch fd
    fn createWatchFd() !posix.fd_t {
        if (comptime builtin.os.tag == .macos) {
            // Create a dedicated kqueue
            const kq = std.c.kqueue();
            if (kq < 0) return error.KqueueCreateFailed;
            return kq;
        } else if (comptime builtin.os.tag == .linux) {
            const fd = std.os.linux.inotify_init1(std.os.linux.IN.NONBLOCK | std.os.linux.IN.CLOEXEC);
            if (fd < 0) return error.InotifyInitFailed;
            return @intCast(fd);
        }
        return error.UnsupportedPlatform;
    }
};

// ============================================================
// Tests
// ============================================================

test "watcher init and deinit" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, null, 500);
    defer w.deinit();

    try std.testing.expectEqual(@as(u32, 500), w.delay_ms);
    try std.testing.expectEqual(@as(u64, 0), w.last_change_ms);
}

test "watcher default delay" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, null, 0);
    defer w.deinit();

    try std.testing.expectEqual(@as(u32, 1000), w.delay_ms);
}

test "shouldIgnore matches patterns" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, "node_modules;.git;*.log", 1000);
    defer w.deinit();

    try std.testing.expect(w.shouldIgnore("src/node_modules/foo"));
    try std.testing.expect(w.shouldIgnore("/path/.git/config"));
    try std.testing.expect(!w.shouldIgnore("src/main.zig"));
}

test "shouldIgnore returns false with no patterns" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, null, 1000);
    defer w.deinit();

    try std.testing.expect(!w.shouldIgnore("anything"));
}

test "checkForChanges returns false when disabled" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, null, 1000);
    w.enabled = false;
    defer w.deinit();

    try std.testing.expect(!w.checkForChanges());
}

test "debounce logic" {
    const allocator = std.testing.allocator;
    var w = Watcher.init(allocator, null, null, 100);
    defer w.deinit();

    // No change recorded → false
    try std.testing.expect(!w.checkDebounce());

    // Set a change in the future (so debounce hasn't elapsed)
    w.last_change_ms = @intCast(std.time.milliTimestamp());
    try std.testing.expect(!w.checkDebounce());

    // Set a change far in the past (debounce elapsed)
    w.last_change_ms = @intCast(std.time.milliTimestamp() - 200);
    try std.testing.expect(w.checkDebounce());

    // After reporting, last_change_ms is reset
    try std.testing.expectEqual(@as(u64, 0), w.last_change_ms);
}

test "resolvePath absolute" {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const result = Watcher.resolvePath("/usr/local/bin", "/home", &buf);
    try std.testing.expect(result != null);
    try std.testing.expectEqualStrings("/usr/local/bin", result.?);
}

test "resolvePath relative" {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const result = Watcher.resolvePath("src/main.zig", "/project", &buf);
    try std.testing.expect(result != null);
    try std.testing.expectEqualStrings("/project/src/main.zig", result.?);
}
