mod connection;
mod message;
mod tcp;
#[cfg(test)]
mod tests;

use crate::msim::MSIMError::ClosedError;
pub use connection::{Connection, TcpConnection};
pub use message::{EventKind, Request};

pub type Result<T> = std::result::Result<T, MSIMError>;

#[derive(thiserror::Error, Debug)]
pub enum MSIMError {
    #[error("IO error: {0}")]
    IOError(std::io::Error),

    /// MSIM connection closed
    #[error("MSIM connection closed")]
    ClosedError,

    /// Error while parsing the MSIM protocol
    #[error("Parse error")]
    ParseError,

    #[error("Unexpected response from MSIM")]
    UnexpectedMessage,

    #[error("Request failed: {0}")]
    RequestFailed(RequestError),

    // MSIM listener thread died unexpectedly
    #[error("Listener died")]
    ListenerDied,
}

#[derive(thiserror::Error, Debug)]
pub enum RequestError {
    #[error("Further unspecified request error")]
    UnspecifiedError,
}

impl From<std::io::Error> for MSIMError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => ClosedError,
            _ => Self::IOError(e),
        }
    }
}
