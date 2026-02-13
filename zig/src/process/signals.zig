const std = @import("std");
const posix = std.posix;

pub const SIGCHLD: u6 = 20; // macOS SIGCHLD
pub const SIGTERM: u6 = 15;
pub const SIGKILL: u6 = 9;
pub const SIGINT: u6 = 2;

/// Result of reaping a child process
pub const ReapResult = struct {
    pid: posix.pid_t,
    exit_code: u8,
    signaled: bool,
    signal: u32,
};

/// Reap all terminated child processes (non-blocking).
/// Returns a list of ReapResults. Caller must free the returned slice.
pub fn reapChildren(allocator: std.mem.Allocator) ![]ReapResult {
    var results: std.ArrayList(ReapResult) = .{};

    while (true) {
        var status: c_int = 0;
        const pid = std.c.waitpid(-1, &status, std.c.W.NOHANG);

        if (pid <= 0) break; // 0 = no more children, -1 = error (ECHILD)

        var reap = ReapResult{
            .pid = pid,
            .exit_code = 0,
            .signaled = false,
            .signal = 0,
        };

        const raw: u32 = @bitCast(status);
        if (std.c.W.IFEXITED(raw)) {
            reap.exit_code = std.c.W.EXITSTATUS(raw);
        } else if (std.c.W.IFSIGNALED(raw)) {
            reap.signaled = true;
            reap.signal = std.c.W.TERMSIG(raw);
        }

        try results.append(allocator, reap);
    }

    return results.toOwnedSlice(allocator);
}

/// Send a signal to a process
pub fn sendSignal(pid: posix.pid_t, sig: u8) !void {
    const result = std.c.kill(pid, @intCast(sig));
    if (result != 0) {
        return error.SignalFailed;
    }
}
