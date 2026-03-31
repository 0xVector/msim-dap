mod server;
mod session;

pub use session::Session;

pub type Result<T> = std::result::Result<T, AdapterError>;

#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("Server error")]
    ServerError(#[from] dap::errors::ServerError),

    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Poisoned lock error")]
    PoisonError,
}
