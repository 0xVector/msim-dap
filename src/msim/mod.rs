//! Module for MSIM protocol and connection handling.
//! Composed of two main parts:
//! - [`frame`]: defines the MSIM protocol messages and parsing logic.
//! - [`connection`] and [`tcp`]: defines the connection handling and request/response logic.
mod connection;
mod frame;
mod tcp;
#[cfg(test)]
mod tests;

use crate::Address;
use frame::CpuId;

pub use connection::{Connection, TcpConnection};
pub use frame::{ArgType, CpuArch, CsrAddress, EventKind, RegisterId, Request, StoppedAtReason};

#[cfg(test)]
pub use connection::RawResponse;
#[cfg(test)]
pub use frame::ResponseStatus;

pub type Result<T> = std::result::Result<T, MsimError>;

/// Errors that can occur while handling MSIM connections and messages.
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
    /// MSIM has terminated
    Terminated,
    /// MSIM has stopped at a specific address due to CPU and a reason
    StoppedAt(CpuId, Address, StoppedAtReason),
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
    pub fn from_raw(
        kind: EventKind,
        arg0: ArgType,
        arg1: ArgType,
        arg2: ArgType,
    ) -> frame::Result<Self> {
        match kind {
            EventKind::Terminated => Ok(Self::Terminated),
            EventKind::StoppedAt => Ok(Self::StoppedAt(arg0, arg1, StoppedAtReason::read(arg2)?)),
        }
    }
}
