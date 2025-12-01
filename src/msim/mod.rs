mod commands;
mod connection;
mod conversion;
mod message;
mod tcp;
#[cfg(test)]
mod tests;

pub use commands::{MsimCommand, MsimResponse};
pub use connection::{MsimConnection, TcpMsimConnection};

pub type Result<T> = std::result::Result<T, MSIMError>;

#[derive(thiserror::Error, Debug)]
pub enum MSIMError {
    #[error("IO error")]
    IOError(#[from] std::io::Error),
}
