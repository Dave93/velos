const std = @import("std");
const posix = std.posix;

/// Event types returned by the platform event loop
pub const EventKind = enum {
    ipc_accept, // New client connection on the IPC socket
    ipc_read, // Data available from an IPC client fd
    pipe_read, // Data available from a child process pipe
    signal, // Signal received (SIGCHLD, SIGTERM, etc.)
    timer, // Timer fired
    ipc_client_hup, // IPC client disconnected
    pipe_hup, // Pipe closed (child process ended output)
};

pub const Event = struct {
    kind: EventKind,
    fd: posix.fd_t, // relevant fd (socket, pipe, etc.)
    signal_number: i32 = 0, // for signal events
};

/// Callback type for event handling
pub const EventCallback = *const fn (event: Event, ctx: *anyopaque) void;

/// Platform Abstraction Layer interface.
/// Each platform (macOS/kqueue, Linux/epoll) implements this.
pub const Pal = struct {
    const Self = @This();

    // Function pointers for vtable dispatch
    addFdFn: *const fn (self_ptr: *anyopaque, fd: posix.fd_t, kind: EventKind) anyerror!void,
    removeFdFn: *const fn (self_ptr: *anyopaque, fd: posix.fd_t) void,
    addSignalFn: *const fn (self_ptr: *anyopaque, sig: u6) anyerror!void,
    pollFn: *const fn (self_ptr: *anyopaque, events: []Event, timeout_ms: ?u32) anyerror!usize,
    deinitFn: *const fn (self_ptr: *anyopaque) void,

    ptr: *anyopaque,

    /// Register a file descriptor for monitoring
    pub fn addFd(self: Self, fd: posix.fd_t, kind: EventKind) !void {
        return self.addFdFn(self.ptr, fd, kind);
    }

    /// Unregister a file descriptor
    pub fn removeFd(self: Self, fd: posix.fd_t) void {
        self.removeFdFn(self.ptr, fd);
    }

    /// Register a signal for monitoring
    pub fn addSignal(self: Self, sig: u6) !void {
        return self.addSignalFn(self.ptr, sig);
    }

    /// Poll for events, returns number of events filled.
    /// timeout_ms: null = block indefinitely, 0 = non-blocking
    pub fn poll(self: Self, events: []Event, timeout_ms: ?u32) !usize {
        return self.pollFn(self.ptr, events, timeout_ms);
    }

    pub fn deinit(self: Self) void {
        self.deinitFn(self.ptr);
    }
};
