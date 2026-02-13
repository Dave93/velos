pub mod error;
pub mod process;
pub mod protocol;

pub use error::VelosError;
pub use process::{ProcessConfig, ProcessStatus};
pub use protocol::LogEntry;
