const std = @import("std");
const posix = std.posix;
const builtin = @import("builtin");

/// Bidirectional IPC channel between daemon and child process.
/// Used for wait_ready (child → parent) and shutdown_with_message (parent → child).
/// Implemented via Unix socketpair.
pub const IpcChannel = struct {
    const Self = @This();

    /// Message length prefix size (u32 little-endian)
    const LENGTH_PREFIX_SIZE = 4;

    /// Maximum message size (64KB)
    const MAX_MSG_SIZE = 64 * 1024;

    /// Daemon-side fd
    parent_fd: posix.fd_t,

    /// Child process-side fd
    child_fd: posix.fd_t,

    /// Create a Unix socketpair for bidirectional IPC.
    pub fn create() !Self {
        var fds: [2]posix.fd_t = undefined;
        const ret = std.c.socketpair(std.c.AF.UNIX, std.c.SOCK.STREAM, 0, &fds);
        if (ret != 0) return error.SocketpairFailed;

        // Set parent_fd to non-blocking
        setNonBlocking(fds[0]);

        return Self{
            .parent_fd = fds[0],
            .child_fd = fds[1],
        };
    }

    /// Close both ends.
    pub fn deinit(self: *Self) void {
        if (self.parent_fd != -1) {
            posix.close(self.parent_fd);
            self.parent_fd = -1;
        }
        if (self.child_fd != -1) {
            posix.close(self.child_fd);
            self.child_fd = -1;
        }
    }

    /// Close child end (call in parent after fork).
    pub fn closeChildEnd(self: *Self) void {
        if (self.child_fd != -1) {
            posix.close(self.child_fd);
            self.child_fd = -1;
        }
    }

    /// Close parent end (call in child after fork, before exec).
    /// The child should then set VELOS_IPC_FD env var to child_fd.
    pub fn closeParentEnd(self: *Self) void {
        if (self.parent_fd != -1) {
            posix.close(self.parent_fd);
            self.parent_fd = -1;
        }
    }

    /// Send a length-prefixed message on the given fd.
    /// Format: length (u32 LE) + data
    pub fn sendMessage(fd: posix.fd_t, msg: []const u8) !void {
        if (msg.len > MAX_MSG_SIZE) return error.MessageTooLarge;

        // Write length prefix
        const len: u32 = @intCast(msg.len);
        const len_bytes = std.mem.toBytes(std.mem.nativeToLittle(u32, len));
        _ = try posix.write(fd, &len_bytes);

        // Write message data
        var written: usize = 0;
        while (written < msg.len) {
            const n = posix.write(fd, msg[written..]) catch |err| {
                if (err == error.WouldBlock) continue;
                return err;
            };
            written += n;
        }
    }

    /// Try to read a length-prefixed message (non-blocking on parent_fd).
    /// Returns null if no data available. Caller owns returned slice.
    pub fn readMessage(allocator: std.mem.Allocator, fd: posix.fd_t) !?[]u8 {
        // Read length prefix (4 bytes)
        var len_buf: [LENGTH_PREFIX_SIZE]u8 = undefined;
        var len_read: usize = 0;

        while (len_read < LENGTH_PREFIX_SIZE) {
            const n = posix.read(fd, len_buf[len_read..]) catch |err| {
                if (err == error.WouldBlock) {
                    if (len_read == 0) return null; // No data at all
                    continue; // Partial read, try again
                }
                return err;
            };
            if (n == 0) return null; // EOF
            len_read += n;
        }

        const msg_len = std.mem.littleToNative(u32, std.mem.bytesToValue(u32, &len_buf));
        if (msg_len > MAX_MSG_SIZE) return error.MessageTooLarge;
        if (msg_len == 0) {
            const empty = try allocator.alloc(u8, 0);
            return empty;
        }

        // Read message body
        const buf = try allocator.alloc(u8, msg_len);
        errdefer allocator.free(buf);

        var body_read: usize = 0;
        while (body_read < msg_len) {
            const n = posix.read(fd, buf[body_read..]) catch |err| {
                if (err == error.WouldBlock) continue;
                return err;
            };
            if (n == 0) return error.UnexpectedEOF;
            body_read += n;
        }

        return buf;
    }

    /// Format the child fd number as a decimal string into the provided buffer.
    /// Returns the slice containing the formatted number.
    pub fn childFdStr(self: *const Self, buf: *[20]u8) []u8 {
        return std.fmt.bufPrint(buf, "{d}", .{self.child_fd}) catch buf[0..0];
    }
};

fn setNonBlocking(fd: posix.fd_t) void {
    const flags = std.c.fcntl(fd, std.c.F.GETFL);
    _ = std.c.fcntl(fd, std.c.F.SETFL, @as(c_int, flags) | @as(c_int, @bitCast(std.c.O{ .NONBLOCK = true })));
}

// ============================================================
// Tests
// ============================================================

test "create and deinit channel" {
    var ch = try IpcChannel.create();
    defer ch.deinit();

    try std.testing.expect(ch.parent_fd >= 0);
    try std.testing.expect(ch.child_fd >= 0);
    try std.testing.expect(ch.parent_fd != ch.child_fd);
}

test "send and receive message" {
    const allocator = std.testing.allocator;
    var ch = try IpcChannel.create();
    defer ch.deinit();

    // Send from child side (blocking fd)
    try IpcChannel.sendMessage(ch.child_fd, "ready");

    // Read from parent side
    const msg = try IpcChannel.readMessage(allocator, ch.parent_fd);
    try std.testing.expect(msg != null);
    defer allocator.free(msg.?);

    try std.testing.expectEqualStrings("ready", msg.?);
}

test "send and receive longer message" {
    const allocator = std.testing.allocator;
    var ch = try IpcChannel.create();
    defer ch.deinit();

    const payload = "{\"type\":\"shutdown\",\"reason\":\"manual stop\"}";
    try IpcChannel.sendMessage(ch.child_fd, payload);

    const msg = try IpcChannel.readMessage(allocator, ch.parent_fd);
    try std.testing.expect(msg != null);
    defer allocator.free(msg.?);

    try std.testing.expectEqualStrings(payload, msg.?);
}

test "read returns null when no data" {
    const allocator = std.testing.allocator;
    var ch = try IpcChannel.create();
    defer ch.deinit();

    // Parent fd is non-blocking, no data written → should return null
    const msg = try IpcChannel.readMessage(allocator, ch.parent_fd);
    try std.testing.expect(msg == null);
}

test "childFdStr formats correctly" {
    var ch = try IpcChannel.create();
    defer ch.deinit();

    var buf: [20]u8 = undefined;
    const s = ch.childFdStr(&buf);
    try std.testing.expect(s.len > 0);

    // Parse back and verify it matches the fd
    const parsed = try std.fmt.parseInt(posix.fd_t, s, 10);
    try std.testing.expectEqual(ch.child_fd, parsed);
}

test "closeChildEnd and closeParentEnd" {
    var ch = try IpcChannel.create();

    // Close child end
    ch.closeChildEnd();
    try std.testing.expectEqual(@as(posix.fd_t, -1), ch.child_fd);

    // Close parent end
    ch.closeParentEnd();
    try std.testing.expectEqual(@as(posix.fd_t, -1), ch.parent_fd);

    // deinit should be safe to call even after closing
    ch.deinit();
}

test "send empty message" {
    const allocator = std.testing.allocator;
    var ch = try IpcChannel.create();
    defer ch.deinit();

    try IpcChannel.sendMessage(ch.child_fd, "");

    const msg = try IpcChannel.readMessage(allocator, ch.parent_fd);
    try std.testing.expect(msg != null);
    defer allocator.free(msg.?);
    try std.testing.expectEqual(@as(usize, 0), msg.?.len);
}

test "multiple messages in sequence" {
    const allocator = std.testing.allocator;
    var ch = try IpcChannel.create();
    defer ch.deinit();

    try IpcChannel.sendMessage(ch.child_fd, "msg1");
    try IpcChannel.sendMessage(ch.child_fd, "msg2");
    try IpcChannel.sendMessage(ch.child_fd, "msg3");

    const m1 = (try IpcChannel.readMessage(allocator, ch.parent_fd)).?;
    defer allocator.free(m1);
    try std.testing.expectEqualStrings("msg1", m1);

    const m2 = (try IpcChannel.readMessage(allocator, ch.parent_fd)).?;
    defer allocator.free(m2);
    try std.testing.expectEqualStrings("msg2", m2);

    const m3 = (try IpcChannel.readMessage(allocator, ch.parent_fd)).?;
    defer allocator.free(m3);
    try std.testing.expectEqualStrings("msg3", m3);
}
