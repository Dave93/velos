use velos_core::VelosError;

pub async fn run(json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    client.save().await?;

    if json {
        println!("{}", serde_json::json!({ "saved": true }));
    } else {
        println!("[velos] Process list saved successfully");
    }

    Ok(())
}
