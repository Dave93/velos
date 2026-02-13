pub mod daemon;
pub mod delete;
pub mod list;
pub mod logs;
pub mod ping;
pub mod start;
pub mod stop;

use velos_client::VelosClient;
use velos_core::VelosError;

/// Helper: connect to the daemon, printing a helpful message if not running.
pub async fn connect() -> Result<VelosClient, VelosError> {
    VelosClient::connect().await
}
