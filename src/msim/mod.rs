mod connection;
mod frame;
mod tcp;
#[cfg(test)]
mod tests;

use crate::Address;
use frame::ArgType;

pub use connection::{Connection, TcpConnection};
pub use frame::{EventKind, Request, StoppedAtReason};

pub type Result<T> = std::result::Result<T, MsimError>;

#[derive(thiserror::Error, Debug)]
pub enum MsimError {
    #[error("IO error: {0}")]
    IOError(std::io::Error),

    /// MSIM connection closed
    #[error("MSIM connection closed")]
    ClosedError,

    /// Error while parsing the MSIM protocol
    #[error("Parse error")]
    ParseError,

    /// Unexpected message from MSIM, e.g. response when no request is pending
    /// This is a fatal error, as it indicates a desync between the adapter and MSIM.
    #[error("Unexpected response from MSIM")]
    UnexpectedMessage,

    /// Recoverable error while processing a request, e.g. invalid params or MSIM internal error.
    #[error("Request failed")]
    RequestFailed,

    /// MSIM listener thread died unexpectedly.
    #[error("Listener died")]
    ListenerDied,
}

// TODO: look I don't love this being separate from other protocol messages but for now im keeping it
/// Interpreted MSIM events
#[derive(Copy, Clone)]
pub enum Event {
    Exited,
    StoppedAt(Address, StoppedAtReason),
}

impl From<std::io::Error> for MsimError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => Self::ClosedError,
            _ => Self::IOError(e),
        }
    }
}

impl Event {
    pub fn from_raw(kind: EventKind, arg0: ArgType, arg1: ArgType) -> frame::Result<Self> {
        match kind {
            EventKind::Exited => Ok(Self::Exited),
            EventKind::StoppedAt => Ok(Self::StoppedAt(arg0, StoppedAtReason::read(arg1)?)),
        }
    }
}
