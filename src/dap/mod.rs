mod context;
mod handler;
mod server;
mod state;

pub use context::Context;
pub use handler::{Handler, Handles};
pub use server::{DapServer, serve, server_from_io, server_from_stdio, server_from_tcp};
pub use state::State;

pub type Result<T> = std::result::Result<T, DapError>;

#[derive(thiserror::Error, Debug)]
pub enum DapError {
    #[error("Server error")]
    ServerError(#[from] dap::errors::ServerError),

    #[error("Unhandled command")]
    UnhandledCommandError,

    #[error("IO error")]
    IoError(#[from] std::io::Error),
}
