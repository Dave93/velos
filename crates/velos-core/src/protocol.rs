use serde::Serialize;

// ============================================================
// Wire format constants
// ============================================================

pub const MAGIC: [u8; 2] = [0x56, 0x10];
pub const VERSION: u8 = 0x01;
pub const HEADER_SIZE: usize = 7;

// ============================================================
// Binary reader/writer (matches Zig protocol helpers)
// ============================================================

pub struct BinaryWriter {
    pub buf: Vec<u8>,
}

impl BinaryWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn write_u8(&mut self, val: u8) {
        self.buf.push(val);
    }

    pub fn write_u32(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u64(&mut self, val: u64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_i32(&mut self, val: i32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_string(&mut self, s: &str) {
        self.write_u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
}

pub struct BinaryReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn read_u8(&mut self) -> Result<u8, crate::VelosError> {
        if self.pos >= self.data.len() {
            return Err(crate::VelosError::ProtocolError("truncated u8".into()));
        }
        let val = self.data[self.pos];
        self.pos += 1;
        Ok(val)
    }

    pub fn read_u32(&mut self) -> Result<u32, crate::VelosError> {
        if self.pos + 4 > self.data.len() {
            return Err(crate::VelosError::ProtocolError("truncated u32".into()));
        }
        let val = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(val)
    }

    pub fn read_i32(&mut self) -> Result<i32, crate::VelosError> {
        if self.pos + 4 > self.data.len() {
            return Err(crate::VelosError::ProtocolError("truncated i32".into()));
        }
        let val = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(val)
    }

    pub fn read_u64(&mut self) -> Result<u64, crate::VelosError> {
        if self.pos + 8 > self.data.len() {
            return Err(crate::VelosError::ProtocolError("truncated u64".into()));
        }
        let val = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(val)
    }

    pub fn read_string(&mut self) -> Result<String, crate::VelosError> {
        let len = self.read_u32()? as usize;
        if self.pos + len > self.data.len() {
            return Err(crate::VelosError::ProtocolError("truncated string".into()));
        }
        let s = std::str::from_utf8(&self.data[self.pos..self.pos + len])
            .map_err(|e| crate::VelosError::ProtocolError(format!("invalid utf8: {e}")))?;
        self.pos += len;
        Ok(s.to_string())
    }

    pub fn read_raw(&mut self) -> Vec<u8> {
        let remaining = &self.data[self.pos..];
        self.pos = self.data.len();
        remaining.to_vec()
    }
}

// ============================================================
// Header encode/decode
// ============================================================

pub fn encode_header(payload_len: u32) -> [u8; HEADER_SIZE] {
    let len_bytes = payload_len.to_le_bytes();
    [
        MAGIC[0], MAGIC[1], VERSION,
        len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3],
    ]
}

pub fn decode_header(buf: &[u8; HEADER_SIZE]) -> Result<u32, crate::VelosError> {
    if buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
        return Err(crate::VelosError::ProtocolError(format!(
            "invalid magic: [{:#04x}, {:#04x}]",
            buf[0], buf[1]
        )));
    }
    if buf[2] != VERSION {
        return Err(crate::VelosError::ProtocolError(format!(
            "unsupported protocol version: {}",
            buf[2]
        )));
    }
    Ok(u32::from_le_bytes([buf[3], buf[4], buf[5], buf[6]]))
}

// ============================================================
// Command codes
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandCode {
    ProcessStart = 0x01,
    ProcessStop = 0x02,
    ProcessRestart = 0x03,
    ProcessDelete = 0x04,
    ProcessList = 0x05,
    ProcessInfo = 0x06,
    ProcessScale = 0x07,
    LogRead = 0x10,
    LogStream = 0x11,
    MetricsGet = 0x20,
    StateSave = 0x30,
    StateLoad = 0x31,
    Ping = 0x40,
    Shutdown = 0x41,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseStatus {
    Ok = 0,
    Error = 1,
    Streaming = 2,
}

impl ResponseStatus {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Ok),
            1 => Some(Self::Error),
            2 => Some(Self::Streaming),
            _ => None,
        }
    }
}

// ============================================================
// Request / Response wire types
// ============================================================

#[derive(Debug, Clone)]
pub struct Request {
    pub id: u32,
    pub command: CommandCode,
    pub payload: Vec<u8>,
}

impl Request {
    pub fn encode(&self) -> Result<Vec<u8>, crate::VelosError> {
        let body_len = 4 + 1 + self.payload.len();
        let header = encode_header(body_len as u32);
        let mut buf = Vec::with_capacity(HEADER_SIZE + body_len);
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&self.id.to_le_bytes());
        buf.push(self.command as u8);
        buf.extend_from_slice(&self.payload);
        Ok(buf)
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub id: u32,
    pub status: ResponseStatus,
    pub payload: Vec<u8>,
}

impl Response {
    pub fn from_body(body: &[u8]) -> Result<Self, crate::VelosError> {
        if body.len() < 5 {
            return Err(crate::VelosError::ProtocolError(
                "response body too short".into(),
            ));
        }
        let id = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let status = ResponseStatus::from_u8(body[4]).ok_or_else(|| {
            crate::VelosError::ProtocolError(format!("unknown response status: {}", body[4]))
        })?;
        let payload = body[5..].to_vec();
        Ok(Self { id, status, payload })
    }

    pub fn error_message(&self) -> String {
        String::from_utf8_lossy(&self.payload).to_string()
    }
}

// ============================================================
// Payload types — binary encode/decode matching Zig
// ============================================================

// --- Start ---

pub struct StartPayload {
    pub name: String,
    pub script: String,
    pub cwd: String,
    pub interpreter: Option<String>,
    pub kill_timeout_ms: u32,
    pub autorestart: bool,
    pub max_restarts: i32,
    pub min_uptime_ms: u64,
    pub restart_delay_ms: u32,
    pub exp_backoff: bool,
    pub max_memory_restart: u64,
    pub watch: bool,
    pub watch_delay_ms: u32,
    pub watch_paths: String,
    pub watch_ignore: String,
    pub cron_restart: String,
    pub wait_ready: bool,
    pub listen_timeout_ms: u32,
    pub shutdown_with_message: bool,
    pub instances: u32,
}

impl StartPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_string(&self.name);
        w.write_string(&self.script);
        w.write_string(&self.cwd);
        w.write_string(self.interpreter.as_deref().unwrap_or(""));
        w.write_u32(self.kill_timeout_ms);
        w.write_u8(if self.autorestart { 1 } else { 0 });
        w.write_i32(self.max_restarts);
        w.write_u64(self.min_uptime_ms);
        w.write_u32(self.restart_delay_ms);
        w.write_u8(if self.exp_backoff { 1 } else { 0 });
        w.write_u64(self.max_memory_restart);
        w.write_u8(if self.watch { 1 } else { 0 });
        w.write_u32(self.watch_delay_ms);
        w.write_string(&self.watch_paths);
        w.write_string(&self.watch_ignore);
        w.write_string(&self.cron_restart);
        w.write_u8(if self.wait_ready { 1 } else { 0 });
        w.write_u32(self.listen_timeout_ms);
        w.write_u8(if self.shutdown_with_message { 1 } else { 0 });
        w.write_u32(self.instances);
        w.buf
    }
}

// --- Scale ---

pub struct ScalePayload {
    pub name: String,
    pub target_count: u32,
}

impl ScalePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_string(&self.name);
        w.write_u32(self.target_count);
        w.buf
    }
}

#[derive(Debug, Clone)]
pub struct ScaleResult {
    pub started: u32,
    pub stopped: u32,
}

impl ScaleResult {
    pub fn decode(data: &[u8]) -> Result<Self, crate::VelosError> {
        let mut r = BinaryReader::new(data);
        Ok(Self {
            started: r.read_u32()?,
            stopped: r.read_u32()?,
        })
    }
}

pub struct StartResult {
    pub id: u32,
}

impl StartResult {
    pub fn decode(data: &[u8]) -> Result<Self, crate::VelosError> {
        let mut r = BinaryReader::new(data);
        Ok(Self { id: r.read_u32()? })
    }
}

// --- Stop ---

pub struct StopPayload {
    pub process_id: u32,
    pub signal: u8,
    pub timeout_ms: u32,
}

impl StopPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_u32(self.process_id);
        w.write_u8(self.signal);
        w.write_u32(self.timeout_ms);
        w.buf
    }
}

// --- Delete ---

pub struct DeletePayload {
    pub process_id: u32,
}

impl DeletePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_u32(self.process_id);
        w.buf
    }
}

// --- List ---

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub id: u32,
    pub name: String,
    pub pid: u32,
    pub status: u8,
    pub memory_bytes: u64,
    pub uptime_ms: u64,
    pub restart_count: u32,
}

impl ProcessInfo {
    pub fn status_str(&self) -> &'static str {
        match self.status {
            0 => "stopped",
            1 => "running",
            2 => "errored",
            3 => "starting",
            _ => "unknown",
        }
    }
}

pub fn decode_process_list(data: &[u8]) -> Result<Vec<ProcessInfo>, crate::VelosError> {
    let mut r = BinaryReader::new(data);
    let count = r.read_u32()? as usize;
    let mut procs = Vec::with_capacity(count);
    for _ in 0..count {
        procs.push(ProcessInfo {
            id: r.read_u32()?,
            name: r.read_string()?,
            pid: r.read_u32()?,
            status: r.read_u8()?,
            memory_bytes: r.read_u64()?,
            uptime_ms: r.read_u64()?,
            restart_count: r.read_u32()?,
        });
    }
    Ok(procs)
}

// --- Restart ---

pub struct RestartPayload {
    pub process_id: u32,
}

impl RestartPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_u32(self.process_id);
        w.buf
    }
}

// --- Info ---

pub struct InfoPayload {
    pub process_id: u32,
}

impl InfoPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_u32(self.process_id);
        w.buf
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessDetail {
    pub id: u32,
    pub name: String,
    pub pid: u32,
    pub status: u8,
    pub memory_bytes: u64,
    pub uptime_ms: u64,
    pub restart_count: u32,
    pub consecutive_crashes: u32,
    pub last_restart_ms: u64,
    pub script: String,
    pub cwd: String,
    pub interpreter: String,
    pub kill_timeout_ms: u32,
    pub autorestart: bool,
    pub max_restarts: i32,
    pub min_uptime_ms: u64,
    pub restart_delay_ms: u32,
    pub exp_backoff: bool,
    pub max_memory_restart: u64,
    pub watch: bool,
    pub cron_restart: String,
    pub wait_ready: bool,
    pub shutdown_with_message: bool,
}

impl ProcessDetail {
    pub fn status_str(&self) -> &'static str {
        match self.status {
            0 => "stopped",
            1 => "running",
            2 => "errored",
            3 => "starting",
            _ => "unknown",
        }
    }
}

/// Decode process detail matching Zig handleProcessInfo encoding order:
/// id(u32) + name(string) + pid(u32) + status(u8) + memory(u64) + uptime(u64)
/// + restarts(u32) + consecutive_crashes(u32) + last_restart_ms(u64)
/// + script(string) + cwd(string) + interpreter(string)
/// + kill_timeout(u32) + autorestart(u8) + max_restarts(i32)
/// + min_uptime_ms(u64) + restart_delay_ms(u32) + exp_backoff(u8)
pub fn decode_process_detail(data: &[u8]) -> Result<ProcessDetail, crate::VelosError> {
    let mut r = BinaryReader::new(data);
    Ok(ProcessDetail {
        id: r.read_u32()?,
        name: r.read_string()?,
        pid: r.read_u32()?,
        status: r.read_u8()?,
        memory_bytes: r.read_u64()?,
        uptime_ms: r.read_u64()?,
        restart_count: r.read_u32()?,
        consecutive_crashes: r.read_u32()?,
        last_restart_ms: r.read_u64()?,
        script: r.read_string()?,
        cwd: r.read_string()?,
        interpreter: r.read_string()?,
        kill_timeout_ms: r.read_u32()?,
        autorestart: r.read_u8()? != 0,
        max_restarts: r.read_i32()?,
        min_uptime_ms: r.read_u64()?,
        restart_delay_ms: r.read_u32()?,
        exp_backoff: r.read_u8()? != 0,
        max_memory_restart: r.read_u64()?,
        watch: r.read_u8()? != 0,
        cron_restart: r.read_string()?,
        wait_ready: r.read_u8()? != 0,
        shutdown_with_message: r.read_u8()? != 0,
    })
}

// --- State Save/Load ---

#[derive(Debug, Clone, Serialize)]
pub struct StateLoadResult {
    pub count: u32,
}

impl StateLoadResult {
    pub fn decode(data: &[u8]) -> Result<Self, crate::VelosError> {
        if data.is_empty() {
            return Ok(Self { count: 0 });
        }
        let mut r = BinaryReader::new(data);
        Ok(Self {
            count: r.read_u32()?,
        })
    }
}

// --- LogRead ---

pub struct LogReadPayload {
    pub process_id: u32,
    pub lines: u32,
}

impl LogReadPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = BinaryWriter::new();
        w.write_u32(self.process_id);
        w.write_u32(self.lines);
        w.buf
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub level: u8,
    pub stream: u8,
    pub message: String,
}

pub fn decode_log_entries(data: &[u8]) -> Result<Vec<LogEntry>, crate::VelosError> {
    let mut r = BinaryReader::new(data);
    let count = r.read_u32()? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(LogEntry {
            timestamp_ms: r.read_u64()?,
            level: r.read_u8()?,
            stream: r.read_u8()?,
            message: r.read_string()?,
        });
    }
    Ok(entries)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let len = 12345u32;
        let header = encode_header(len);
        assert_eq!(header[0], 0x56);
        assert_eq!(header[1], 0x10);
        assert_eq!(header[2], VERSION);
        let decoded = decode_header(&header).unwrap();
        assert_eq!(decoded, len);
    }

    #[test]
    fn test_header_bad_magic() {
        let mut header = encode_header(100);
        header[0] = 0xFF;
        assert!(decode_header(&header).is_err());
    }

    #[test]
    fn test_request_encode() {
        let req = Request {
            id: 1,
            command: CommandCode::Ping,
            payload: vec![],
        };
        let bytes = req.encode().unwrap();
        assert_eq!(bytes.len(), 7 + 4 + 1);
        assert_eq!(bytes[0], 0x56);
        assert_eq!(bytes[1], 0x10);
    }

    #[test]
    fn test_response_from_body() {
        let mut body = Vec::new();
        body.extend_from_slice(&42u32.to_le_bytes());
        body.push(ResponseStatus::Ok as u8);
        body.extend_from_slice(b"pong");

        let resp = Response::from_body(&body).unwrap();
        assert_eq!(resp.id, 42);
        assert_eq!(resp.status, ResponseStatus::Ok);
        assert_eq!(&resp.payload, b"pong");
    }

    #[test]
    fn test_binary_writer_reader_roundtrip() {
        let mut w = BinaryWriter::new();
        w.write_u32(0xDEADBEEF);
        w.write_string("test_string");
        w.write_u8(0x42);
        w.write_u64(9999999);

        let mut r = BinaryReader::new(&w.buf);
        assert_eq!(r.read_u32().unwrap(), 0xDEADBEEF);
        assert_eq!(r.read_string().unwrap(), "test_string");
        assert_eq!(r.read_u8().unwrap(), 0x42);
        assert_eq!(r.read_u64().unwrap(), 9999999);
    }

    #[test]
    fn test_start_payload_encode() {
        let payload = StartPayload {
            name: "myapp".into(),
            script: "app.js".into(),
            cwd: "/tmp".into(),
            interpreter: None,
            kill_timeout_ms: 5000,
            autorestart: true,
            max_restarts: 15,
            min_uptime_ms: 1000,
            restart_delay_ms: 100,
            exp_backoff: false,
            max_memory_restart: 0,
            watch: false,
            watch_delay_ms: 0,
            watch_paths: String::new(),
            watch_ignore: String::new(),
            cron_restart: String::new(),
            wait_ready: false,
            listen_timeout_ms: 8000,
            shutdown_with_message: false,
            instances: 1,
        };
        let bytes = payload.encode();

        let mut r = BinaryReader::new(&bytes);
        assert_eq!(r.read_string().unwrap(), "myapp");
        assert_eq!(r.read_string().unwrap(), "app.js");
        assert_eq!(r.read_string().unwrap(), "/tmp");
        assert_eq!(r.read_string().unwrap(), ""); // no interpreter
        assert_eq!(r.read_u32().unwrap(), 5000);
        assert_eq!(r.read_u8().unwrap(), 1); // autorestart
        assert_eq!(r.read_i32().unwrap(), 15); // max_restarts
        assert_eq!(r.read_u64().unwrap(), 1000); // min_uptime_ms
        assert_eq!(r.read_u32().unwrap(), 100); // restart_delay_ms
        assert_eq!(r.read_u8().unwrap(), 0); // exp_backoff
        assert_eq!(r.read_u64().unwrap(), 0); // max_memory_restart
        assert_eq!(r.read_u8().unwrap(), 0); // watch
        assert_eq!(r.read_u32().unwrap(), 0); // watch_delay_ms
        assert_eq!(r.read_string().unwrap(), ""); // watch_paths
        assert_eq!(r.read_string().unwrap(), ""); // watch_ignore
        assert_eq!(r.read_string().unwrap(), ""); // cron_restart
        assert_eq!(r.read_u8().unwrap(), 0); // wait_ready
        assert_eq!(r.read_u32().unwrap(), 8000); // listen_timeout_ms
        assert_eq!(r.read_u8().unwrap(), 0); // shutdown_with_message
        assert_eq!(r.read_u32().unwrap(), 1); // instances
    }

    #[test]
    fn test_process_detail_decode() {
        // Matches Zig handleProcessInfo encoding order
        let mut w = BinaryWriter::new();
        w.write_u32(1);          // id
        w.write_string("myapp"); // name
        w.write_u32(1234);      // pid
        w.write_u8(1);          // status = running
        w.write_u64(50 * 1024 * 1024); // memory_bytes
        w.write_u64(120000);    // uptime_ms
        w.write_u32(3);         // restart_count
        w.write_u32(2);         // consecutive_crashes
        w.write_u64(100000);    // last_restart_ms
        w.write_string("app.js"); // config.script
        w.write_string("/tmp");   // config.cwd
        w.write_string("node");   // config.interpreter
        w.write_u32(5000);       // config.kill_timeout_ms
        w.write_u8(1);           // config.autorestart
        w.write_i32(15);         // config.max_restarts
        w.write_u64(1000);       // config.min_uptime_ms
        w.write_u32(100);        // config.restart_delay_ms
        w.write_u8(0);           // config.exp_backoff
        w.write_u64(150 * 1024 * 1024); // max_memory_restart
        w.write_u8(1);           // watch
        w.write_string("0 0 * * *"); // cron_restart
        w.write_u8(1);           // wait_ready
        w.write_u8(0);           // shutdown_with_message

        let detail = decode_process_detail(&w.buf).unwrap();
        assert_eq!(detail.id, 1);
        assert_eq!(detail.name, "myapp");
        assert_eq!(detail.pid, 1234);
        assert_eq!(detail.status_str(), "running");
        assert_eq!(detail.memory_bytes, 50 * 1024 * 1024);
        assert_eq!(detail.uptime_ms, 120000);
        assert_eq!(detail.restart_count, 3);
        assert_eq!(detail.consecutive_crashes, 2);
        assert_eq!(detail.last_restart_ms, 100000);
        assert_eq!(detail.script, "app.js");
        assert_eq!(detail.cwd, "/tmp");
        assert_eq!(detail.interpreter, "node");
        assert_eq!(detail.kill_timeout_ms, 5000);
        assert!(detail.autorestart);
        assert_eq!(detail.max_restarts, 15);
        assert_eq!(detail.min_uptime_ms, 1000);
        assert_eq!(detail.restart_delay_ms, 100);
        assert!(!detail.exp_backoff);
        assert_eq!(detail.max_memory_restart, 150 * 1024 * 1024);
        assert!(detail.watch);
        assert_eq!(detail.cron_restart, "0 0 * * *");
        assert!(detail.wait_ready);
        assert!(!detail.shutdown_with_message);
    }

    #[test]
    fn test_process_list_decode() {
        let mut w = BinaryWriter::new();
        w.write_u32(1); // count
        w.write_u32(1); // id
        w.write_string("myapp"); // name
        w.write_u32(1234); // pid
        w.write_u8(1); // status = running
        w.write_u64(1024 * 1024); // memory
        w.write_u64(60000); // uptime
        w.write_u32(0); // restarts

        let procs = decode_process_list(&w.buf).unwrap();
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].id, 1);
        assert_eq!(procs[0].name, "myapp");
        assert_eq!(procs[0].pid, 1234);
        assert_eq!(procs[0].status_str(), "running");
    }
}
