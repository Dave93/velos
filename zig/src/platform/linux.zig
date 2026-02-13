const std = @import("std");
const posix = std.posix;
const linux = std.os.linux;
const pal = @import("pal.zig");

/// Linux epoll-based event loop implementation
pub const EpollLoop = struct {
    const Self = @This();
    const MAX_EVENTS = 64;

    epoll_fd: posix.fd_t,
    signal_fd: posix.fd_t,
    // Accumulated signal mask for signalfd
    signal_mask: linux.sigset_t,
    // Track which fds map to which EventKind
    fd_kinds: std.AutoHashMap(posix.fd_t, pal.EventKind),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) !Self {
        const epfd = linux.epoll_create1(linux.EPOLL.CLOEXEC);
        if (@as(isize, @bitCast(epfd)) < 0) return error.EpollCreateFailed;

        // Create signalfd for signal handling
        var mask: linux.sigset_t = std.mem.zeroes(linux.sigset_t);
        const sfd = linux.signalfd(-1, &mask, linux.SFD.NONBLOCK | linux.SFD.CLOEXEC);
        if (@as(isize, @bitCast(sfd)) < 0) {
            _ = linux.close(@intCast(epfd));
            return error.SignalFdCreateFailed;
        }

        // Add signalfd to epoll
        var ev = linux.epoll_event{
            .events = linux.EPOLL.IN,
            .data = .{ .fd = @intCast(sfd) },
        };
        const ret = linux.epoll_ctl(@intCast(epfd), linux.EPOLL.CTL_ADD, @intCast(sfd), &ev);
        if (@as(isize, @bitCast(ret)) < 0) {
            _ = linux.close(@intCast(sfd));
            _ = linux.close(@intCast(epfd));
            return error.EpollCtlFailed;
        }

        return Self{
            .epoll_fd = @intCast(epfd),
            .signal_fd = @intCast(sfd),
            .signal_mask = std.mem.zeroes(linux.sigset_t),
            .fd_kinds = std.AutoHashMap(posix.fd_t, pal.EventKind).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        posix.close(self.signal_fd);
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

    fn addSignalImpl(self_ptr: *anyopaque, sig: u6) anyerror!void {
        const self: *Self = @ptrCast(@alignCast(self_ptr));

        // Add signal to accumulated mask
        const sig_int: usize = @intCast(sig);
        const word_index = sig_int / @bitSizeOf(usize);
        const bit_index: u5 = @intCast(sig_int % @bitSizeOf(usize));
        if (word_index < self.signal_mask.len) {
            self.signal_mask[word_index] |= @as(usize, 1) << bit_index;
        }

        // Block the signal from default handling
        _ = linux.sigprocmask(linux.SIG.BLOCK, &self.signal_mask, null);

        // Update signalfd with full accumulated mask
        _ = linux.signalfd(self.signal_fd, &self.signal_mask, linux.SFD.NONBLOCK | linux.SFD.CLOEXEC);

        try self.fd_kinds.put(@intCast(sig), pal.EventKind.signal);
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

            // Check if this is the signalfd
            if (fd == self.signal_fd) {
                // Read signalfd_siginfo structs
                var siginfo: linux.signalfd_siginfo = undefined;
                const bytes = posix.read(self.signal_fd, std.mem.asBytes(&siginfo)) catch continue;
                if (bytes == @sizeOf(linux.signalfd_siginfo)) {
                    events[count] = pal.Event{
                        .kind = .signal,
                        .fd = -1,
                        .signal_number = @intCast(siginfo.signo),
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
