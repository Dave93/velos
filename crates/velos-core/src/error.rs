/// Velos error types
#[derive(Debug, thiserror::Error)]
pub enum VelosError {
    #[error("daemon is not running")]
    DaemonNotRunning,

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("connection timeout")]
    ConnectionTimeout,

    #[error("process not found: {0}")]
    ProcessNotFound(String),

    #[error("protocol error: {0}")]
    ProtocolError(String),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
