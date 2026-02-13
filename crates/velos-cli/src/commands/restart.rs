use velos_core::VelosError;

pub async fn run(name_or_id: String, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;

    if name_or_id == "all" {
        let procs = client.list().await?;
        if procs.is_empty() {
            if json {
                println!("[]");
            } else {
                println!("[velos] No processes to restart");
            }
            return Ok(());
        }
        let mut restarted = Vec::new();
        for p in &procs {
            client.restart(p.id).await?;
            restarted.push(serde_json::json!({ "id": p.id, "name": p.name }));
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&restarted).unwrap_or_default());
        } else {
            for p in &procs {
                println!("[velos] Restarted '{}' (id={})", p.name, p.id);
            }
        }
        return Ok(());
    }

    let id = super::resolve_id(&mut client, &name_or_id).await?;
    client.restart(id).await?;

    if json {
        println!("{}", serde_json::json!({ "restarted": id }));
    } else {
        println!("[velos] Restarted process '{}' (id={})", name_or_id, id);
    }

    Ok(())
}
