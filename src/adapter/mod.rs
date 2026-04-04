mod server;
mod session;

pub use session::Session;

pub type Result<T> = std::result::Result<T, AdapterError>;

#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("Server error")]
    Server(#[from] dap::errors::ServerError),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Poisoned lock error")]
    PoisonedLock,
}
