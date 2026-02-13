use velos_core::VelosError;

pub async fn run(name_or_id: String, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let id = super::resolve_id(&mut client, &name_or_id).await?;

    client.delete(id).await?;

    if json {
        println!("{}", serde_json::json!({ "deleted": id }));
    } else {
        println!("[velos] Deleted process '{}' (id={})", name_or_id, id);
    }

    Ok(())
}
