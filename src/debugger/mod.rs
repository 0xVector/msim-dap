//! Debugger module implements the main event loop and debugging event handlers.
//! It is composed of the following submodules:
//! - `core`: contains the main [`Debugger`] struct and its implementation.
//! - `events`: defines the events that the [`Debugger`] can handle and emit.
//! - `requests`: defines the requests that the [`Debugger`] can handle and emit.
mod core;
mod events;
mod requests;

use crate::adapter;

pub use core::Debugger;

/// Result type for the debugger module.
pub type Result<T> = std::result::Result<T, DebuggerError>;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that can occur in the debugger module, aggregating
/// error types from lower layers.
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

    #[error("Debugging tool disconnected")]
    DapDisconnected,

    // Recoverable error
    #[error("Request failed")]
    RequestFailed(#[source] AnyError),
}

// TODO: maybe even some AdapterError::ServerError are recoverable
// TODO: maybe some dwarf unrecoverable actually?
