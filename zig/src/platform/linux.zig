const std = @import("std");
const posix = std.posix;
const linux = std.os.linux;
const pal = @import("pal.zig");

/// Global self-pipe for signal delivery (set by EpollLoop.init)
var g_signal_pipe_w: posix.fd_t = -1;

/// Signal handler that writes signal number to the self-pipe.
fn signalHandler(sig: c_int) callconv(.c) void {
    if (g_signal_pipe_w == -1) return;
    const sig_byte: [1]u8 = .{@intCast(@as(c_uint, @bitCast(sig)))};
    _ = posix.write(g_signal_pipe_w, &sig_byte) catch {};
}

/// Linux epoll-based event loop implementation using self-pipe for signals.
pub const EpollLoop = struct {
    const Self = @This();
    const MAX_EVENTS = 64;

    epoll_fd: posix.fd_t,
    signal_pipe_r: posix.fd_t,
    signal_pipe_w: posix.fd_t,
    // Track which fds map to which EventKind
    fd_kinds: std.AutoHashMap(posix.fd_t, pal.EventKind),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) !Self {
        const epfd = linux.epoll_create1(linux.EPOLL.CLOEXEC);
        if (@as(isize, @bitCast(epfd)) < 0) return error.EpollCreateFailed;

        // Create self-pipe for signal delivery
        var pipe_fds: [2]posix.fd_t = undefined;
        const pipe_ret = linux.pipe2(&pipe_fds, .{ .NONBLOCK = true, .CLOEXEC = true });
        if (@as(isize, @bitCast(pipe_ret)) < 0) {
            _ = linux.close(@intCast(epfd));
            return error.PipeCreateFailed;
        }

        // Set global write end for signal handler
        g_signal_pipe_w = pipe_fds[1];

        // Add read end of pipe to epoll
        var ev = linux.epoll_event{
            .events = linux.EPOLL.IN,
            .data = .{ .fd = pipe_fds[0] },
        };
        const ctl_ret = linux.epoll_ctl(@intCast(epfd), linux.EPOLL.CTL_ADD, @intCast(pipe_fds[0]), &ev);
        if (@as(isize, @bitCast(ctl_ret)) < 0) {
            _ = linux.close(@intCast(pipe_fds[0]));
            _ = linux.close(@intCast(pipe_fds[1]));
            _ = linux.close(@intCast(epfd));
            return error.EpollCtlFailed;
        }

        return Self{
            .epoll_fd = @intCast(epfd),
            .signal_pipe_r = pipe_fds[0],
            .signal_pipe_w = pipe_fds[1],
            .fd_kinds = std.AutoHashMap(posix.fd_t, pal.EventKind).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        g_signal_pipe_w = -1;
        posix.close(self.signal_pipe_r);
        posix.close(self.signal_pipe_w);
        posix.close(self.epoll_fd);
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

        var ev = linux.epoll_event{
            .events = linux.EPOLL.IN | linux.EPOLL.HUP | linux.EPOLL.RDHUP,
            .data = .{ .fd = fd },
        };

        const ret = linux.epoll_ctl(@intCast(self.epoll_fd), linux.EPOLL.CTL_ADD, @intCast(fd), &ev);
        if (@as(isize, @bitCast(ret)) < 0) return error.EpollCtlFailed;
        try self.fd_kinds.put(fd, kind);
    }

    fn removeFdImpl(self_ptr: *anyopaque, fd: posix.fd_t) void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        _ = linux.epoll_ctl(@intCast(self.epoll_fd), linux.EPOLL.CTL_DEL, @intCast(fd), null);
        _ = self.fd_kinds.remove(fd);
    }

    fn addSignalImpl(_: *anyopaque, sig: u6) anyerror!void {
        // Install signal handler that writes to the self-pipe
        var sa: std.c.Sigaction = .{
            .handler = .{ .handler = signalHandler },
            .mask = std.mem.zeroes(std.c.sigset_t),
            .flags = std.c.SA.RESTART | std.c.SA.NOCLDSTOP,
        };
        _ = std.c.sigaction(@intCast(sig), &sa, null);
    }

    fn pollImpl(self_ptr: *anyopaque, events: []pal.Event, timeout_ms: ?u32) anyerror!usize {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        const max_events = @min(events.len, MAX_EVENTS);
        var epoll_events: [MAX_EVENTS]linux.epoll_event = undefined;

        const timeout: i32 = if (timeout_ms) |ms| @intCast(ms) else -1;

        const n = linux.epoll_wait(
            @intCast(self.epoll_fd),
            @ptrCast(epoll_events[0..max_events]),
            @intCast(max_events),
            timeout,
        );

        if (@as(isize, @bitCast(n)) < 0) return error.EpollWaitFailed;
        const count_raw: usize = @intCast(n);

        var count: usize = 0;
        for (epoll_events[0..count_raw]) |ev| {
            const fd = ev.data.fd;

            // Check if this is the signal pipe
            if (fd == self.signal_pipe_r) {
                // Read all pending signal bytes
                var sig_buf: [64]u8 = undefined;
                const bytes = posix.read(self.signal_pipe_r, &sig_buf) catch 0;
                for (sig_buf[0..bytes]) |sig_byte| {
                    if (count >= events.len) break;
                    events[count] = pal.Event{
                        .kind = .signal,
                        .fd = -1,
                        .signal_number = @intCast(sig_byte),
                    };
                    count += 1;
                }
                continue;
            }

            const kind = self.fd_kinds.get(fd) orelse continue;

            // Check for HUP
            if (ev.events & (linux.EPOLL.HUP | linux.EPOLL.RDHUP) != 0) {
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
