pub mod commands;
pub mod connection;

pub use commands::VelosClient;
pub use connection::VelosConnection;

/// Default socket path: ~/.velos/velos.sock
pub fn default_socket_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join(".velos")
        .join("velos.sock")
}

/// Default PID file path: ~/.velos/velos.pid
pub fn default_pid_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join(".velos")
        .join("velos.pid")
}

/// Check if the daemon is likely running by checking PID file existence
/// and whether the process is alive.
pub fn is_daemon_running() -> bool {
    let pid_path = default_pid_path();
    let Ok(content) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = content.trim().parse::<i32>() else {
        return false;
    };
    // Check if process is alive via kill(pid, 0)
    unsafe { libc::kill(pid, 0) == 0 }
}
