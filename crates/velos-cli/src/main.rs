mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "velos",
    version = "0.1.0-dev",
    about = "High-performance AI-friendly process manager"
)]
struct Cli {
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
    /// Start a process
    Start {
        /// Script/command to run
        script: String,
        /// Process name (defaults to script basename)
        #[arg(short, long)]
        name: Option<String>,
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
    /// List all processes
    #[command(alias = "ls")]
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
    },
    /// Delete a process
    Delete {
        /// Process name or ID
        name_or_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Ping the daemon (IPC)
    Ping,
    /// Ping the Zig core (FFI, for testing)
    PingFfi,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Daemon { socket, state_dir } => commands::daemon::run(socket, state_dir),
        Commands::Start { script, name, json } => commands::start::run(script, name, json).await,
        Commands::Stop { name_or_id, json } => commands::stop::run(name_or_id, json).await,
        Commands::List { json } => commands::list::run(json).await,
        Commands::Logs { name, lines, json } => commands::logs::run(name, lines, json).await,
        Commands::Delete { name_or_id, json } => commands::delete::run(name_or_id, json).await,
        Commands::Ping => commands::ping::run().await,
        Commands::PingFfi => {
            let response = velos_ffi::ping();
            println!("{}", response);
            Ok(())
        }
    };

    if let Err(e) = result {
        match &e {
            velos_core::VelosError::DaemonNotRunning => {
                eprintln!("Error: Daemon is not running. Start it with: velos-daemon");
            }
            _ => {
                eprintln!("Error: {e}");
            }
        }
        std::process::exit(1);
    }
}
