const std = @import("std");

/// A fixed-capacity ring buffer storing the last N log lines.
/// Thread-safe via a mutex for concurrent read/write access.
pub const RingBuffer = struct {
    const Self = @This();

    pub const Entry = struct {
        timestamp_ms: u64,
        level: u8, // 0=debug, 1=info, 2=warn, 3=error
        stream: u8, // 0=stdout, 1=stderr
        message: []const u8, // owned copy
    };

    entries: []?Entry,
    capacity: u32,
    head: u32, // next write position
    count: u32, // current number of entries
    allocator: std.mem.Allocator,
    mutex: std.Thread.Mutex,

    pub fn init(allocator: std.mem.Allocator, capacity: u32) !Self {
        const entries = try allocator.alloc(?Entry, capacity);
        @memset(entries, null);
        return Self{
            .entries = entries,
            .capacity = capacity,
            .head = 0,
            .count = 0,
            .allocator = allocator,
            .mutex = .{},
        };
    }

    pub fn deinit(self: *Self) void {
        for (self.entries) |maybe_entry| {
            if (maybe_entry) |entry| {
                self.allocator.free(entry.message);
            }
        }
        self.allocator.free(self.entries);
    }

    /// Push a new entry into the ring buffer. Overwrites the oldest if full.
    pub fn push(self: *Self, timestamp_ms: u64, level: u8, stream: u8, message: []const u8) !void {
        self.mutex.lock();
        defer self.mutex.unlock();

        // Free the old entry at head position if it exists
        if (self.entries[self.head]) |old| {
            self.allocator.free(old.message);
        }

        // Copy message
        const msg_copy = try self.allocator.dupe(u8, message);

        self.entries[self.head] = Entry{
            .timestamp_ms = timestamp_ms,
            .level = level,
            .stream = stream,
            .message = msg_copy,
        };

        self.head = (self.head + 1) % self.capacity;
        if (self.count < self.capacity) {
            self.count += 1;
        }
    }

    /// Read the last `n` entries (oldest first). Caller must free returned slice.
    pub fn readLast(self: *Self, n: u32) ![]Entry {
        self.mutex.lock();
        defer self.mutex.unlock();

        const actual_n = @min(n, self.count);
        if (actual_n == 0) return try self.allocator.alloc(Entry, 0);

        const result = try self.allocator.alloc(Entry, actual_n);

        // Calculate start position: oldest of the last `actual_n` entries
        const start = if (self.count < self.capacity)
            self.head - actual_n
        else
            (self.head + self.capacity - actual_n) % self.capacity;

        for (0..actual_n) |i| {
            const idx = (start + @as(u32, @intCast(i))) % self.capacity;
            const entry = self.entries[idx].?;
            result[i] = Entry{
                .timestamp_ms = entry.timestamp_ms,
                .level = entry.level,
                .stream = entry.stream,
                .message = entry.message, // shared reference, valid as long as ring buffer lives
            };
        }

        return result;
    }

    pub fn freeReadResult(self: *Self, result: []Entry) void {
        self.allocator.free(result);
    }
};

test "ring buffer basic operations" {
    var rb = try RingBuffer.init(std.testing.allocator, 4);
    defer rb.deinit();

    try rb.push(1000, 1, 0, "hello");
    try rb.push(2000, 1, 0, "world");

    const entries = try rb.readLast(10);
    defer rb.freeReadResult(entries);

    try std.testing.expectEqual(@as(usize, 2), entries.len);
    try std.testing.expectEqualStrings("hello", entries[0].message);
    try std.testing.expectEqualStrings("world", entries[1].message);
}

test "ring buffer wraps around" {
    var rb = try RingBuffer.init(std.testing.allocator, 3);
    defer rb.deinit();

    try rb.push(1, 1, 0, "a");
    try rb.push(2, 1, 0, "b");
    try rb.push(3, 1, 0, "c");
    try rb.push(4, 1, 0, "d"); // overwrites "a"

    const entries = try rb.readLast(3);
    defer rb.freeReadResult(entries);

    try std.testing.expectEqual(@as(usize, 3), entries.len);
    try std.testing.expectEqualStrings("b", entries[0].message);
    try std.testing.expectEqualStrings("c", entries[1].message);
    try std.testing.expectEqualStrings("d", entries[2].message);
}
