//! DAP specific functionality, providing the [`Session`] abstraction over the DAP server.
mod server;
mod session;

pub use session::Session;

/// Result type for adapter operations
pub type Result<T> = std::result::Result<T, AdapterError>;

/// Errors that can occur in the adapter
#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("Server error: {0}")]
    Server(#[from] dap::errors::ServerError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Poisoned lock error")]
    PoisonedLock,
}
