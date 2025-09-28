pub type AdapterResult<T> = Result<T, AdapterError>;

#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("Server error")]
    ServerError(#[from] dap::errors::ServerError),
    
    #[error("Unhandled command")]
    UnhandledCommandError,

    #[error("IO error")]
    IoError(#[from] std::io::Error)

    // #[error("Missing command")]
    // MissingCommandError,
}