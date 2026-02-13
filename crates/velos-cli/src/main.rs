mod commands;

use clap::{Parser, Subcommand};

fn version_string() -> &'static str {
    Box::leak(
        format!(
            "{} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH,
            std::env::consts::OS,
        )
        .into_boxed_str(),
    )
}

#[derive(Parser)]
#[command(
    name = "velos",
    version = version_string(),
    about = "High-performance AI-friendly process manager",
    after_help = "\x1b[1mExamples:\x1b[0m
  velos daemon                Start the daemon
  velos start app.js          Start a process
  velos start app.js -i 4     Start 4 instances (cluster mode)
  velos list                  List all processes
  velos info app              Show detailed process info
  velos logs app --level error  Show error logs only
  velos logs app --summary    Show log health summary
  velos scale app 8           Scale to 8 instances
  velos save                  Save process list
  velos resurrect             Restore saved processes
  velos monit                 TUI monitoring dashboard"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the daemon in foreground
    Daemon {
        /// Custom socket path
        #[arg(long)]
        socket: Option<String>,
        /// Custom state directory
        #[arg(long)]
        state_dir: Option<String>,
    },
    /// Start a process (or processes from config)
    Start {
        /// Script/command to run
        script: Option<String>,
        /// Process name (defaults to script basename)
        #[arg(short, long)]
        name: Option<String>,
        /// Load processes from TOML config file
        #[arg(long)]
        config: Option<String>,
        /// Enable watch mode (restart on file changes)
        #[arg(long)]
        watch: bool,
        /// Max restart attempts (-1 = unlimited)
        #[arg(long)]
        max_restarts: Option<i32>,
        /// Disable autorestart
        #[arg(long)]
        no_autorestart: bool,
        /// Max memory before restart (e.g. "150M", "1G")
        #[arg(long)]
        max_memory: Option<String>,
        /// Cron expression for periodic restart
        #[arg(long)]
        cron_restart: Option<String>,
        /// Wait for ready signal from process
        #[arg(long)]
        wait_ready: bool,
        /// Send shutdown message via IPC instead of SIGTERM
        #[arg(long)]
        shutdown_with_message: bool,
        /// Number of instances for cluster mode (number or "max" for CPU count)
        #[arg(short, long)]
        instances: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Stop a running process
    Stop {
        /// Process name or ID
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Restart a running process
    Restart {
        /// Process name, ID, or "all"
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Gracefully reload a process
    Reload {
        /// Process name, ID, or "all"
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List all processes
    #[command(alias = "ls")]
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Compact AI-friendly output
        #[arg(long)]
        ai: bool,
    },
    /// Show detailed process info
    Info {
        /// Process name or ID
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Compact AI-friendly output
        #[arg(long)]
        ai: bool,
    },
    /// Show process logs
    Logs {
        /// Process name or ID
        name: String,
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Compact AI-friendly output
        #[arg(long)]
        ai: bool,
        /// Filter by regex pattern
        #[arg(long)]
        grep: Option<String>,
        /// Filter by level (comma-separated: error,warn)
        #[arg(long)]
        level: Option<String>,
        /// Show logs since time (e.g. "1h", "30m", "2d")
        #[arg(long)]
        since: Option<String>,
        /// Show logs until time
        #[arg(long)]
        until: Option<String>,
        /// Deduplicate similar messages
        #[arg(long)]
        dedupe: bool,
        /// Show summary (health score, patterns, anomalies)
        #[arg(long)]
        summary: bool,
    },
    /// Delete a process
    Delete {
        /// Process name or ID
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Save current process list
    Save {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Restore previously saved processes
    Resurrect {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Flush log files
    Flush {
        /// Process name or ID (all if omitted)
        name_or_id: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Start Prometheus metrics exporter
    Metrics {
        /// HTTP port for /metrics endpoint
        #[arg(short, long, default_value = "9615")]
        port: u16,
        /// OpenTelemetry OTLP endpoint (e.g. http://localhost:4318)
        #[arg(long)]
        otel_endpoint: Option<String>,
    },
    /// Start REST API server with WebSocket support
    Api {
        /// HTTP port for the API server
        #[arg(short, long, default_value = "3100")]
        port: u16,
        /// API token for authentication (optional)
        #[arg(long)]
        token: Option<String>,
    },
    /// Scale cluster instances (set count, +N, -N, or max)
    Scale {
        /// Process name (base name of the cluster)
        name: String,
        /// Target instance count (number, +N, -N, or "max")
        count: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Generate startup script for init system (systemd/launchd/openrc)
    Startup,
    /// Remove startup script
    Unstartup,
    /// TUI monitoring dashboard
    Monit,
    /// Start MCP server (stdio transport for AI agents)
    McpServer,
    /// Ping the daemon (IPC)
    Ping,
    /// Ping the Zig core (FFI, for testing)
    PingFfi,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Daemon { socket, state_dir } => commands::daemon::run(socket, state_dir),
        Commands::Start {
            script,
            name,
            json,
            config,
            watch,
            max_restarts,
            no_autorestart,
            max_memory,
            cron_restart,
            wait_ready,
            shutdown_with_message,
            instances,
        } => {
            commands::start::run(commands::start::StartArgs {
                script,
                name,
                json,
                config,
                watch,
                max_restarts,
                no_autorestart,
                max_memory,
                cron_restart,
                wait_ready,
                shutdown_with_message,
                instances,
            })
            .await
        }
        Commands::Stop { name_or_id, json } => commands::stop::run(name_or_id, json).await,
        Commands::Restart { name_or_id, json } => commands::restart::run(name_or_id, json).await,
        Commands::Reload { name_or_id, json } => commands::reload::run(name_or_id, json).await,
        Commands::List { json, ai } => commands::list::run(json, ai).await,
        Commands::Info { name_or_id, json, ai } => commands::info::run(name_or_id, json, ai).await,
        Commands::Logs {
            name,
            lines,
            json,
            ai,
            grep,
            level,
            since,
            until,
            dedupe,
            summary,
        } => {
            commands::logs::run(commands::logs::LogsArgs {
                name,
                lines,
                json,
                ai,
                grep,
                level,
                since,
                until,
                dedupe,
                summary,
            })
            .await
        }
        Commands::Delete { name_or_id, json } => commands::delete::run(name_or_id, json).await,
        Commands::Save { json } => commands::save::run(json).await,
        Commands::Resurrect { json } => commands::resurrect::run(json).await,
        Commands::Flush { name_or_id, json } => commands::flush::run(name_or_id, json).await,
        Commands::Scale { name, count, json } => commands::scale::run(name, count, json).await,
        Commands::Api { port, token } => commands::api::run(port, token).await,
        Commands::Metrics { port, otel_endpoint } => commands::metrics::run(port, otel_endpoint).await,
        Commands::Startup => commands::startup::run_startup().await,
        Commands::Unstartup => commands::startup::run_unstartup().await,
        Commands::Monit => commands::monit::run().await,
        Commands::McpServer => {
            let server = velos_mcp::server::McpServer::new();
            server.run().await.map_err(|e| {
                velos_core::VelosError::ProtocolError(e.to_string())
            })
        }
        Commands::Ping => commands::ping::run().await,
        Commands::PingFfi => {
            let response = velos_ffi::ping();
            println!("{}", response);
            Ok(())
        }
        Commands::Completions { shell } => commands::completions::run(shell),
    };

    if let Err(e) = result {
        match &e {
            velos_core::VelosError::DaemonNotRunning => {
                eprintln!("Error: Daemon is not running.");
                eprintln!("  Start it with: velos daemon");
            }
            velos_core::VelosError::ConnectionFailed(msg) => {
                eprintln!("Error: Cannot connect to daemon: {msg}");
                eprintln!("  Is the daemon running? Start with: velos daemon");
            }
            velos_core::VelosError::ConnectionTimeout => {
                eprintln!("Error: Connection to daemon timed out.");
                eprintln!("  Check if the daemon is responsive: velos ping");
            }
            velos_core::VelosError::ProcessNotFound(name) => {
                eprintln!("Error: Process '{name}' not found.");
                eprintln!("  Run 'velos list' to see running processes.");
            }
            velos_core::VelosError::ProtocolError(msg) => {
                eprintln!("Error: Protocol error: {msg}");
                eprintln!("  This may indicate a version mismatch. Try restarting the daemon.");
            }
            _ => {
                eprintln!("Error: {e}");
            }
        }
        std::process::exit(1);
    }
}
