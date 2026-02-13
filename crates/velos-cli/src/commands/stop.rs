use velos_core::VelosError;

pub async fn run(name_or_id: String, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let ids = super::resolve_ids(&mut client, &name_or_id).await?;

    for id in &ids {
        client.stop(*id).await?;
    }

    if json {
        let stopped: Vec<_> = ids.iter().map(|id| serde_json::json!({ "stopped": id })).collect();
        println!("{}", serde_json::to_string(&stopped).unwrap_or_default());
    } else if ids.len() > 1 {
        println!(
            "[velos] Stopped {} instances of '{}'",
            ids.len(),
            name_or_id
        );
    } else {
        println!("[velos] Stopped process '{}' (id={})", name_or_id, ids[0]);
    }

    Ok(())
}
