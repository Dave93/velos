use std::path::Path;

use velos_core::protocol::*;
use velos_core::VelosError;

use crate::connection::VelosConnection;

/// High-level client for the Velos daemon.
pub struct VelosClient {
    conn: VelosConnection,
}

impl VelosClient {
    pub async fn connect() -> Result<Self, VelosError> {
        let conn = VelosConnection::connect_default().await?;
        Ok(Self { conn })
    }

    pub async fn connect_to(socket_path: &Path) -> Result<Self, VelosError> {
        let conn = VelosConnection::connect(socket_path).await?;
        Ok(Self { conn })
    }

    /// Start a new process. Returns the assigned process ID.
    pub async fn start(&mut self, payload: StartPayload) -> Result<StartResult, VelosError> {
        let resp = self
            .conn
            .request(CommandCode::ProcessStart, payload.encode())
            .await?;
        self.check_response(&resp)?;
        StartResult::decode(&resp.payload)
    }

    /// Stop a process by ID.
    pub async fn stop(&mut self, id: u32) -> Result<(), VelosError> {
        let payload = StopPayload {
            process_id: id,
            signal: 15, // SIGTERM
            timeout_ms: 5000,
        };
        let resp = self
            .conn
            .request(CommandCode::ProcessStop, payload.encode())
            .await?;
        self.check_response(&resp)
    }

    /// List all processes.
    pub async fn list(&mut self) -> Result<Vec<ProcessInfo>, VelosError> {
        let resp = self
            .conn
            .request(CommandCode::ProcessList, Vec::new())
            .await?;
        self.check_response(&resp)?;
        decode_process_list(&resp.payload)
    }

    /// Read log entries for a process.
    pub async fn logs(&mut self, id: u32, lines: u32) -> Result<Vec<LogEntry>, VelosError> {
        let payload = LogReadPayload {
            process_id: id,
            lines,
        };
        let resp = self
            .conn
            .request(CommandCode::LogRead, payload.encode())
            .await?;
        self.check_response(&resp)?;
        decode_log_entries(&resp.payload)
    }

    /// Delete a process.
    pub async fn delete(&mut self, id: u32) -> Result<(), VelosError> {
        let payload = DeletePayload { process_id: id };
        let resp = self
            .conn
            .request(CommandCode::ProcessDelete, payload.encode())
            .await?;
        self.check_response(&resp)
    }

    /// Ping the daemon. Returns the raw pong message.
    pub async fn ping(&mut self) -> Result<String, VelosError> {
        let resp = self
            .conn
            .request(CommandCode::Ping, Vec::new())
            .await?;
        self.check_response(&resp)?;
        Ok(String::from_utf8_lossy(&resp.payload).to_string())
    }

    /// Shutdown the daemon.
    pub async fn shutdown(&mut self) -> Result<(), VelosError> {
        let resp = self
            .conn
            .request(CommandCode::Shutdown, Vec::new())
            .await?;
        self.check_response(&resp)
    }

    fn check_response(&self, resp: &Response) -> Result<(), VelosError> {
        match resp.status {
            ResponseStatus::Ok | ResponseStatus::Streaming => Ok(()),
            ResponseStatus::Error => {
                Err(VelosError::ProtocolError(resp.error_message()))
            }
        }
    }
}
