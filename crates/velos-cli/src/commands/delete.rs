use velos_core::VelosError;

pub async fn run(name_or_id: String, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;

    let id: u32 = name_or_id
        .parse()
        .map_err(|_| VelosError::ProcessNotFound(name_or_id.clone()))?;

    client.delete(id).await?;

    if json {
        println!("{}", serde_json::json!({ "deleted": id }));
    } else {
        println!("[velos] Deleted process '{}'", name_or_id);
    }

    Ok(())
}
