use velos_core::VelosError;

pub async fn run(json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let result = client.resurrect().await?;

    if json {
        println!("{}", serde_json::json!({ "restored": result.count }));
    } else if result.count == 0 {
        println!("[velos] No saved processes to restore");
    } else {
        println!("[velos] Restored {} process(es)", result.count);
    }

    Ok(())
}
