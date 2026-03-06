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

    // Start Telegram callback poller if configured
    let poller_child = start_telegram_poller();

    eprintln!("[velos-daemon] Initialized. Entering event loop.");

    velos_ffi::daemon_run().map_err(|code| {
        VelosError::Io(std::io::Error::other(format!(
            "daemon_run failed with code {code}"
        )))
    })?;

    eprintln!("[velos-daemon] Event loop exited. Shutting down.");

    // Stop Telegram poller
    if let Some(mut child) = poller_child {
        eprintln!("[velos-daemon] Stopping Telegram poller...");
        let _ = child.kill();
        let _ = child.wait();
    }

    let _ = velos_ffi::daemon_shutdown();
    Ok(())
}

/// Spawn `velos telegram-poller` as a child process if Telegram is configured.
fn start_telegram_poller() -> Option<std::process::Child> {
    let config = super::config::load_global_config().ok()?;
    let tg = config.notifications.as_ref()?.telegram.as_ref()?;

    if tg.bot_token.is_empty() || tg.chat_id.is_empty() {
        return None;
    }

    let exe = std::env::current_exe().ok()?;

    // Log agent activity to file for debugging
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".velos")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::File::create(log_dir.join("telegram-poller.log")).ok()?;

    match std::process::Command::new(&exe)
        .arg("telegram-poller")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .spawn()
    {
        Ok(child) => {
            eprintln!("[velos-daemon] Telegram poller started (pid {})", child.id());
            Some(child)
        }
        Err(e) => {
            eprintln!("[velos-daemon] Failed to start Telegram poller: {e}");
            None
        }
    }
}
