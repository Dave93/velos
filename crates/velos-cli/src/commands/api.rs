use velos_core::VelosError;

pub async fn run(port: u16, token: Option<String>) -> Result<(), VelosError> {
    velos_api::start_server(port, token).await
}
