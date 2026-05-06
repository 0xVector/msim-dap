//! DAP session abstraction, built on top of the DAP server.
use super::server::{post_server_background, server_from_io, server_from_stdio, server_from_tcp};
use super::{AdapterError, Result};
use crate::DebugEventSender;
use dap::base_message::Sendable;
use dap::server::ServerOutput;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};

pub type DapServerOutput = Arc<Mutex<ServerOutput<Box<dyn Write + Send>>>>;

/// DAP session abstraction.
/// Encapsulates the DAP server output and is capable of sending messages to the client.
/// It does not handle incoming requests, as those are sent to the provided channel from the server listener thread.
pub struct Session {
    server_output: DapServerOutput,
}

// DAP session abstraction
impl Session {
    fn new(server_output: DapServerOutput) -> Self {
        Self { server_output }
    }

    /// Create a DAP session from any reader and writer,
    /// posting the server to a background thread and sending received requests to the provided channel.
    #[allow(unused)] // Maybe in the future, nice for parity with server_from_...
    pub fn session_from_io<R, W>(r: R, w: W, tx: DebugEventSender) -> Self
    where
        R: Read + 'static + Send,
        W: Write + 'static + Send,
    {
        let server = server_from_io(r, w);
        let session = Self::new(Arc::clone(&server.output));
        post_server_background(server, tx);
        session
    }
    
    /// Create a DAP session that reads from stdin and writes to stdout,
    /// posting the server to a background thread and sending received requests to the provided channel.
    pub fn session_from_stdio(tx: DebugEventSender) -> Self {
        let server = server_from_stdio();
        let session = Self::new(Arc::clone(&server.output));
        post_server_background(server, tx);
        session
    }

    /// Create a DAP session that listens on a TCP socket,
    /// posting the server to a background thread and sending received requests to the provided channel.
    pub fn session_from_tcp(address: impl ToSocketAddrs, tx: DebugEventSender) -> Result<Self> {
        let server = server_from_tcp(address)?;
        let session = Self::new(Arc::clone(&server.output));
        post_server_background(server, tx);
        Ok(session)
    }

    /// Send a message to the client through the DAP server output.
    /// This blocks the current thread until the message is sent.
    pub fn send(&self, what: Sendable) -> Result<()> {
        Ok(self.server_output.lock()?.send(what)?)
    }
}

impl<T> From<std::sync::PoisonError<T>> for AdapterError {
    fn from(_err: std::sync::PoisonError<T>) -> Self {
        Self::PoisonedLock
    }
}
