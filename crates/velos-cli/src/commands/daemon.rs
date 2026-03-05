use velos_core::VelosError;

pub fn run(socket_path: Option<String>, state_dir: Option<String>) -> Result<(), VelosError> {
    eprintln!("[velos-daemon] Starting...");

    velos_ffi::daemon_init(socket_path.as_deref(), state_dir.as_deref()).map_err(|code| {
        VelosError::Io(std::io::Error::other(format!(
            "daemon_init failed with code {code}"
        )))
    })?;

    // Pass our binary path to Zig so it can fork+exec for crash notifications
    if let Ok(exe) = std::env::current_exe() {
        velos_ffi::set_notify_binary(&exe.to_string_lossy());
    }

    eprintln!("[velos-daemon] Initialized. Entering event loop.");

    velos_ffi::daemon_run().map_err(|code| {
        VelosError::Io(std::io::Error::other(format!(
            "daemon_run failed with code {code}"
        )))
    })?;

    eprintln!("[velos-daemon] Event loop exited. Shutting down.");

    let _ = velos_ffi::daemon_shutdown();
    Ok(())
}
