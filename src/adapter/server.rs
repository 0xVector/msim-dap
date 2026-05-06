//! DAP server functions for constructing and running a DAP server
use super::{AdapterError, Result};

use crate::DebugEvent;
use crate::DebugEventSender;
use dap::server::Server;
use std::io::{BufReader, BufWriter, Read, Write, stdin, stdout};
use std::net::{TcpListener, ToSocketAddrs};

/// Type alias for a DAP server with boxed dynamic reader and writer,
/// allowing for flexible I/O sources (e.g., stdio, TCP streams).
pub type DapServer = Server<Box<dyn Read + Send>, Box<dyn Write + Send>>;

/// Create a DAP server from any reader and writer
pub fn server_from_io<R, W>(r: R, w: W) -> DapServer
where
    R: Read + 'static + Send,
    W: Write + 'static + Send,
{
    Server::new(BufReader::new(Box::new(r)), BufWriter::new(Box::new(w)))
}

/// Create a DAP server that reads from stdin and writes to stdout
pub fn server_from_stdio() -> DapServer {
    server_from_io(stdin(), stdout())
}

/// Create a DAP server that listens on a TCP socket
pub fn server_from_tcp(address: impl ToSocketAddrs) -> Result<DapServer> {
    let listener = TcpListener::bind(address)?;
    let (stream, _addr) = listener.accept()?;
    Ok(server_from_io(stream.try_clone()?, stream))
}

/// Run the DAP server in a background thread, sending received requests to the provided channel.
pub fn post_server_background(mut server: DapServer, tx: DebugEventSender) {
    std::thread::spawn(move || {
        loop {
            match server.poll_request() {
                Ok(Some(req)) => {
                    if tx.send(Ok(DebugEvent::DapRequest(req))).is_err() {
                        break; // rx dropped, exit
                    }
                }

                Ok(None) => break, // got EOF, just exit

                Err(e) => {
                    tx.send(Err(AdapterError::from(e).into())).ok();
                    break; // got some DAP error, exit after sending it
                }
            }
        }
    });
}
