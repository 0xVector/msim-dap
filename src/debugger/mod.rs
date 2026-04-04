mod core;

use crate::adapter;
pub use core::Debugger;

pub type Result<T> = std::result::Result<T, DebuggerError>;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

#[derive(thiserror::Error, Debug)]
pub enum DebuggerError {
    // Fatal errors
    #[error("Target session lost")]
    SessionLost,

    #[error("DAP error: {0}")]
    DapError(#[from] adapter::AdapterError),

    /// Forwarded fatal errors from listeners
    #[error("Received fatal error from listeners: {0}")]
    ReceivedFatalError(#[source] AnyError),

    // Recoverable error
    #[error("Request failed")]
    RequestFailed(#[source] AnyError),
}

// TODO: maybe even some AdapterError::ServerError are recoverable
// TODO: maybe some dwarf unrecoverable actually?
