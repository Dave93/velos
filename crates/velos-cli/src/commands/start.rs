use velos_core::protocol::StartPayload;
use velos_core::VelosError;

pub async fn run(script: String, name: Option<String>, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;

    let process_name = name.unwrap_or_else(|| {
        std::path::Path::new(&script)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("app")
            .to_string()
    });

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let payload = StartPayload {
        name: process_name.clone(),
        script,
        cwd,
        interpreter: None,
        kill_timeout_ms: 5000,
        autorestart: false,
    };

    let result = client.start(payload).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "id": result.id,
                "name": process_name,
            })
        );
    } else {
        println!("[velos] Started '{}' (id={})", process_name, result.id);
    }

    Ok(())
}
