use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use velos_core::protocol::{self, CommandCode, Request, Response, HEADER_SIZE};
use velos_core::VelosError;

/// Low-level IPC connection to the Velos daemon.
pub struct VelosConnection {
    stream: UnixStream,
    socket_path: PathBuf,
    next_id: AtomicU32,
}

impl VelosConnection {
    /// Connect to the daemon at the given socket path.
    pub async fn connect(socket_path: &Path) -> Result<Self, VelosError> {
        let stream = UnixStream::connect(socket_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                || e.kind() == std::io::ErrorKind::NotFound
            {
                VelosError::DaemonNotRunning
            } else {
                VelosError::ConnectionFailed(e.to_string())
            }
        })?;
        Ok(Self {
            stream,
            socket_path: socket_path.to_path_buf(),
            next_id: AtomicU32::new(1),
        })
    }

    /// Connect using the default socket path.
    pub async fn connect_default() -> Result<Self, VelosError> {
        Self::connect(&crate::default_socket_path()).await
    }

    /// Get the socket path this connection uses.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Allocate the next request ID.
    fn next_request_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a request and wait for the response.
    pub async fn request(
        &mut self,
        command: CommandCode,
        payload: Vec<u8>,
    ) -> Result<Response, VelosError> {
        let req = Request {
            id: self.next_request_id(),
            command,
            payload,
        };
        self.send_request(&req).await?;
        self.read_response().await
    }

    /// Send a raw request to the daemon.
    async fn send_request(&mut self, req: &Request) -> Result<(), VelosError> {
        let bytes = req.encode()?;
        self.stream.write_all(&bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Read a response from the daemon.
    async fn read_response(&mut self) -> Result<Response, VelosError> {
        // Read 7-byte header
        let mut header_buf = [0u8; HEADER_SIZE];
        self.stream.read_exact(&mut header_buf).await?;
        let payload_len = protocol::decode_header(&header_buf)?;

        // Read payload
        let mut body = vec![0u8; payload_len as usize];
        self.stream.read_exact(&mut body).await?;

        Response::from_body(&body)
    }
}
