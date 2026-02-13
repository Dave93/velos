use velos_core::protocol::StartPayload;
use velos_core::VelosError;

pub struct StartArgs {
    pub script: Option<String>,
    pub name: Option<String>,
    pub json: bool,
    pub config: Option<String>,
    pub watch: bool,
    pub max_restarts: Option<i32>,
    pub no_autorestart: bool,
    pub max_memory: Option<String>,
    pub cron_restart: Option<String>,
    pub wait_ready: bool,
    pub shutdown_with_message: bool,
    pub instances: Option<String>,
}

pub async fn run(args: StartArgs) -> Result<(), VelosError> {
    // If --config provided, load from config file
    if let Some(config_path) = &args.config {
        return run_from_config(config_path, &args).await;
    }

    let script = args.script.ok_or_else(|| {
        VelosError::ProtocolError("script argument is required when not using --config".into())
    })?;

    let mut client = super::connect().await?;

    let process_name = args.name.unwrap_or_else(|| {
        std::path::Path::new(&script)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("app")
            .to_string()
    });

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let autorestart = !args.no_autorestart;
    let max_restarts = args.max_restarts.unwrap_or(15);

    let max_memory_restart = if let Some(ref mem_str) = args.max_memory {
        velos_config::parse_memory_string(mem_str)
            .map_err(|e| VelosError::ProtocolError(format!("invalid memory format: {e}")))?
    } else {
        0
    };

    let instances = parse_instances(&args.instances)?;

    let payload = StartPayload {
        name: process_name.clone(),
        script,
        cwd,
        interpreter: None,
        kill_timeout_ms: 5000,
        autorestart,
        max_restarts,
        min_uptime_ms: 1000,
        restart_delay_ms: 0,
        exp_backoff: false,
        max_memory_restart,
        watch: args.watch,
        watch_delay_ms: 1000,
        watch_paths: String::new(),
        watch_ignore: String::new(),
        cron_restart: args.cron_restart.clone().unwrap_or_default(),
        wait_ready: args.wait_ready,
        listen_timeout_ms: 8000,
        shutdown_with_message: args.shutdown_with_message,
        instances,
    };

    let result = client.start(payload).await?;

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "id": result.id,
                "name": process_name,
                "instances": instances,
            })
        );
    } else if instances > 1 {
        println!(
            "[velos] Started '{}' in cluster mode ({} instances, first id={})",
            process_name, instances, result.id
        );
    } else {
        println!("[velos] Started '{}' (id={})", process_name, result.id);
    }

    Ok(())
}

async fn run_from_config(config_path: &str, args: &StartArgs) -> Result<(), VelosError> {
    let path = std::path::Path::new(config_path);
    let config = velos_config::load(path)
        .map_err(|e| VelosError::ProtocolError(format!("config error: {e}")))?;

    let mut client = super::connect().await?;

    for (key, app) in &config.apps {
        let app_name = app.name.clone().unwrap_or_else(|| key.clone());
        let autorestart = if args.no_autorestart {
            false
        } else {
            app.autorestart
        };
        let max_restarts = args.max_restarts.unwrap_or(app.max_restarts);

        let max_memory_restart = if let Some(ref mem) = app.max_memory_restart {
            velos_config::parse_memory_string(mem).unwrap_or(0)
        } else {
            0
        };
        let watch_paths = app.watch_paths.join(";");
        let watch_ignore = app.watch_ignore.join(";");

        let payload = StartPayload {
            name: app_name.clone(),
            script: app.script.clone(),
            cwd: app.cwd.clone().unwrap_or_else(|| ".".into()),
            interpreter: app.interpreter.clone(),
            kill_timeout_ms: app.kill_timeout as u32,
            autorestart,
            max_restarts,
            min_uptime_ms: app.min_uptime,
            restart_delay_ms: app.restart_delay as u32,
            exp_backoff: app.exp_backoff_restart_delay,
            max_memory_restart,
            watch: app.watch,
            watch_delay_ms: app.watch_delay as u32,
            watch_paths,
            watch_ignore,
            cron_restart: app.cron_restart.clone().unwrap_or_default(),
            wait_ready: false,
            listen_timeout_ms: 8000,
            shutdown_with_message: false,
            instances: app.instances,
        };

        let result = client.start(payload).await?;

        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "id": result.id,
                    "name": app_name,
                })
            );
        } else {
            println!("[velos] Started '{}' (id={})", app_name, result.id);
        }
    }

    Ok(())
}

fn parse_instances(instances_arg: &Option<String>) -> Result<u32, VelosError> {
    match instances_arg {
        None => Ok(1),
        Some(s) => {
            let s = s.trim();
            if s.eq_ignore_ascii_case("max") || s == "0" {
                let cpus = std::thread::available_parallelism()
                    .map(|n| n.get() as u32)
                    .unwrap_or(1);
                Ok(cpus)
            } else {
                s.parse::<u32>().map_err(|_| {
                    VelosError::ProtocolError(format!(
                        "invalid instances value '{}': use a number or 'max'",
                        s
                    ))
                })
            }
        }
    }
}
