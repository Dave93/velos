use std::path::PathBuf;
use velos_core::VelosError;

fn detect_init_system() -> &'static str {
    match std::env::consts::OS {
        "macos" => "launchd",
        "linux" => {
            if PathBuf::from("/run/systemd/system").exists() {
                "systemd"
            } else if PathBuf::from("/sbin/openrc").exists() {
                "openrc"
            } else {
                "unknown"
            }
        }
        other => {
            eprintln!("[velos] Unsupported OS: {other}");
            "unknown"
        }
    }
}

fn velos_binary_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "/usr/local/bin/velos".to_string())
}

fn generate_systemd_unit(velos_bin: &str) -> String {
    format!(
        r#"[Unit]
Description=Velos Process Manager
After=network.target

[Service]
Type=simple
ExecStart={velos_bin} daemon
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
LimitNOFILE=65536
Environment=HOME=%h

[Install]
WantedBy=default.target
"#
    )
}

fn generate_launchd_plist(velos_bin: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.velos.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{velos_bin}</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/velos-daemon.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/velos-daemon.stderr.log</string>
</dict>
</plist>
"#
    )
}

fn generate_openrc_script(velos_bin: &str) -> String {
    format!(
        r#"#!/sbin/openrc-run

name="velos"
description="Velos Process Manager"
command="{velos_bin}"
command_args="daemon"
command_background=true
pidfile="/run/${{RC_SVCNAME}}.pid"

depend() {{
    need net
    after firewall
}}
"#
    )
}

fn systemd_service_path() -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let dir = home.join(".config/systemd/user");
    std::fs::create_dir_all(&dir).ok();
    dir.join("velos-daemon.service")
}

fn launchd_plist_path() -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let dir = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&dir).ok();
    dir.join("com.velos.daemon.plist")
}

fn openrc_service_path() -> PathBuf {
    PathBuf::from("/etc/init.d/velos")
}

pub async fn run_startup() -> Result<(), VelosError> {
    let init = detect_init_system();
    let velos_bin = velos_binary_path();

    match init {
        "systemd" => {
            let unit = generate_systemd_unit(&velos_bin);
            let path = systemd_service_path();
            std::fs::write(&path, unit)?;
            println!("[velos] Systemd unit written to {}", path.display());
            println!();
            println!("  Enable and start:");
            println!("    systemctl --user daemon-reload");
            println!("    systemctl --user enable --now velos-daemon");
            println!();
            println!("  Check status:");
            println!("    systemctl --user status velos-daemon");
        }
        "launchd" => {
            let plist = generate_launchd_plist(&velos_bin);
            let path = launchd_plist_path();
            std::fs::write(&path, plist)?;
            println!("[velos] Launchd plist written to {}", path.display());
            println!();
            println!("  Load and start:");
            println!("    launchctl load {}", path.display());
            println!();
            println!("  Check status:");
            println!("    launchctl list | grep velos");
        }
        "openrc" => {
            let script = generate_openrc_script(&velos_bin);
            let path = openrc_service_path();
            if std::fs::write(&path, &script).is_err() {
                eprintln!(
                    "[velos] Cannot write to {}. Try running with sudo.",
                    path.display()
                );
                let fallback = PathBuf::from("/tmp/velos-openrc-init");
                std::fs::write(&fallback, script)?;
                println!(
                    "[velos] OpenRC script written to {} — copy it to {} manually",
                    fallback.display(),
                    path.display()
                );
            } else {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
                }
                println!("[velos] OpenRC script written to {}", path.display());
            }
            println!();
            println!("  Enable and start:");
            println!("    rc-update add velos default");
            println!("    rc-service velos start");
        }
        _ => {
            eprintln!("[velos] No supported init system detected (systemd, launchd, openrc)");
            std::process::exit(1);
        }
    }

    // Auto-save process list after startup setup
    match super::connect().await {
        Ok(mut client) => {
            if client.save().await.is_ok() {
                println!();
                println!("[velos] Process list saved automatically");
            }
        }
        Err(_) => {
            println!();
            println!("[velos] Daemon not running — skipping auto-save");
        }
    }

    Ok(())
}

pub async fn run_unstartup() -> Result<(), VelosError> {
    let init = detect_init_system();
    let mut removed = false;

    match init {
        "systemd" => {
            let path = systemd_service_path();
            if path.exists() {
                std::fs::remove_file(&path)?;
                removed = true;
                println!("[velos] Removed {}", path.display());
                println!();
                println!("  Disable the service:");
                println!("    systemctl --user disable velos-daemon");
                println!("    systemctl --user daemon-reload");
            }
        }
        "launchd" => {
            let path = launchd_plist_path();
            if path.exists() {
                println!("  Unloading service first:");
                println!("    launchctl unload {}", path.display());
                // Try to unload before removing
                std::process::Command::new("launchctl")
                    .args(["unload", &path.to_string_lossy()])
                    .output()
                    .ok();
                std::fs::remove_file(&path)?;
                removed = true;
                println!("[velos] Removed {}", path.display());
            }
        }
        "openrc" => {
            let path = openrc_service_path();
            if path.exists() {
                if std::fs::remove_file(&path).is_err() {
                    eprintln!(
                        "[velos] Cannot remove {}. Try running with sudo.",
                        path.display()
                    );
                    std::process::exit(1);
                }
                removed = true;
                println!("[velos] Removed {}", path.display());
                println!();
                println!("  Disable the service:");
                println!("    rc-update del velos default");
            }
        }
        _ => {
            eprintln!("[velos] No supported init system detected");
            std::process::exit(1);
        }
    }

    if !removed {
        println!("[velos] No startup configuration found — nothing to remove");
    }

    Ok(())
}
