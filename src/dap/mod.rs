mod context;
mod handler;
mod server;
mod state;

pub use handler::Handler;
pub use server::{serve, server_from_stdio, server_from_tcp};

pub type Result<T> = std::result::Result<T, DapError>;

#[derive(thiserror::Error, Debug)]
pub enum DapError {
    #[error("Server error")]
    ServerError(#[from] dap::errors::ServerError),

    #[error("Unhandled command")]
    UnhandledCommandError(String),

    #[error("IO error")]
    IoError(#[from] std::io::Error),
}
