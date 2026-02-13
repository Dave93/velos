use velos_core::VelosError;

pub async fn run() -> Result<(), VelosError> {
    let mut client = super::connect().await?;
    let msg = client.ping().await?;
    println!("[velos] {msg}");
    Ok(())
}
