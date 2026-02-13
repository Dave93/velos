const std = @import("std");
const posix = std.posix;
const pal = @import("pal.zig");

const c = @cImport({
    @cInclude("sys/event.h");
    @cInclude("sys/time.h");
    @cInclude("signal.h");
});

/// macOS kqueue-based event loop implementation
pub const KqueueLoop = struct {
    const Self = @This();
    const MAX_KEVENTS = 64;

    kq: posix.fd_t,
    // Track which fds map to which EventKind
    fd_kinds: std.AutoHashMap(posix.fd_t, pal.EventKind),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) !Self {
        const kq = try posix.kqueue();
        return Self{
            .kq = kq,
            .fd_kinds = std.AutoHashMap(posix.fd_t, pal.EventKind).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        posix.close(self.kq);
        self.fd_kinds.deinit();
    }

    pub fn asPal(self: *Self) pal.Pal {
        return pal.Pal{
            .addFdFn = addFdImpl,
            .removeFdFn = removeFdImpl,
            .addSignalFn = addSignalImpl,
            .pollFn = pollImpl,
            .deinitFn = deinitImpl,
            .ptr = @ptrCast(self),
        };
    }

    fn addFdImpl(self_ptr: *anyopaque, fd: posix.fd_t, kind: pal.EventKind) anyerror!void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        var changelist: [1]posix.Kevent = undefined;
        changelist[0] = posix.Kevent{
            .ident = @intCast(fd),
            .filter = std.c.EVFILT.READ,
            .flags = std.c.EV.ADD | std.c.EV.ENABLE,
            .fflags = 0,
            .data = 0,
            .udata = 0,
        };

        _ = try posix.kevent(self.kq, &changelist, &[0]posix.Kevent{}, null);
        try self.fd_kinds.put(fd, kind);
    }

    fn removeFdImpl(self_ptr: *anyopaque, fd: posix.fd_t) void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        var changelist: [1]posix.Kevent = undefined;
        changelist[0] = posix.Kevent{
            .ident = @intCast(fd),
            .filter = std.c.EVFILT.READ,
            .flags = std.c.EV.DELETE,
            .fflags = 0,
            .data = 0,
            .udata = 0,
        };

        _ = posix.kevent(self.kq, &changelist, &[0]posix.Kevent{}, null) catch {};
        _ = self.fd_kinds.remove(fd);
    }

    fn addSignalImpl(self_ptr: *anyopaque, sig: u6) anyerror!void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        // Block the signal from default handling
        var mask: std.c.sigset_t = std.mem.zeroes(std.c.sigset_t);
        _ = std.c.sigaddset(&mask, @intCast(sig));
        _ = std.c.sigprocmask(std.c.SIG.BLOCK, &mask, null);

        var changelist: [1]posix.Kevent = undefined;
        changelist[0] = posix.Kevent{
            .ident = @intCast(sig),
            .filter = std.c.EVFILT.SIGNAL,
            .flags = std.c.EV.ADD | std.c.EV.ENABLE,
            .fflags = 0,
            .data = 0,
            .udata = 0,
        };

        _ = try posix.kevent(self.kq, &changelist, &[0]posix.Kevent{}, null);
        try self.fd_kinds.put(@intCast(sig), pal.EventKind.signal);
    }

    fn pollImpl(self_ptr: *anyopaque, events: []pal.Event, timeout_ms: ?u32) anyerror!usize {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        const max_events = @min(events.len, MAX_KEVENTS);
        var kevents: [MAX_KEVENTS]posix.Kevent = undefined;

        const timeout: ?posix.timespec = if (timeout_ms) |ms| posix.timespec{
            .sec = @intCast(ms / 1000),
            .nsec = @intCast(@as(u64, ms % 1000) * 1_000_000),
        } else null;

        const timeout_ptr: ?*const posix.timespec = if (timeout) |*t| t else null;

        const n = try posix.kevent(self.kq, &[0]posix.Kevent{}, kevents[0..max_events], timeout_ptr);

        var count: usize = 0;
        for (kevents[0..n]) |kev| {
            const fd: posix.fd_t = @intCast(kev.ident);

            if (kev.filter == std.c.EVFILT.SIGNAL) {
                events[count] = pal.Event{
                    .kind = .signal,
                    .fd = -1,
                    .signal_number = @intCast(kev.ident),
                };
                count += 1;
                continue;
            }

            const kind = self.fd_kinds.get(fd) orelse continue;

            // Check for EOF/HUP
            if (kev.flags & std.c.EV.EOF != 0) {
                events[count] = pal.Event{
                    .kind = switch (kind) {
                        .ipc_read, .ipc_accept => .ipc_client_hup,
                        .pipe_read => .pipe_hup,
                        else => kind,
                    },
                    .fd = fd,
                };
                count += 1;
                continue;
            }

            events[count] = pal.Event{
                .kind = kind,
                .fd = fd,
            };
            count += 1;
        }

        return count;
    }

    fn deinitImpl(self_ptr: *anyopaque) void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));
        self.deinit();
    }
};
