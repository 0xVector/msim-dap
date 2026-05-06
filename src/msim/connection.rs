//! Connection management for MSIM.
//! This module defines the `Connection` trait for sending requests and receiving responses,
//! as well as a TCP-based implementation of this trait.
//! The TCP implementation uses a background thread to read from the TCP stream in a blocking
//! manner, dispatching responses to an internal channel for the `send()` method and events to the main event channel.

use super::frame::{FrameError, Inbound, Request, ResponseStatus};
use super::tcp::connect;
use super::{Event, MsimError, Result};
use crate::{DebugEvent, DebugEventSender};
use std::net::TcpStream;

/// Trait representing a connection to MSIM,
/// capable of sending requests and receiving responses.
pub trait Connection {
    /// Send a request to MSIM and wait for the corresponding response.
    /// This method blocks until a response is received.
    fn send(&mut self, command: Request) -> Result<RawResponse>;
}

/// Raw deserialized response from MSIM with uninterpreted arguments.
#[derive(Debug, Clone, Copy)]
pub struct RawResponse {
    pub status: ResponseStatus,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
}

type ResponseTx = std::sync::mpsc::Sender<RawResponse>;
type ResponseRx = std::sync::mpsc::Receiver<RawResponse>;

/// TCP-based implementation of the Connection trait.
/// Uses a background thread to continuously read from the TCP stream
/// and dispatch messages to the appropriate channels.
/// Responses are sent to an internal channel for the [`TcpConnection::send()`] method,
/// while events are sent to the main event channel for the rest of the system.
pub struct TcpConnection {
    stream: TcpStream,
    resp_rx: ResponseRx, // Internal channel for receiving responses from the background thread.
}

impl TcpConnection {
    const fn new(stream: TcpStream, resp_rx: ResponseRx) -> Self {
        Self { stream, resp_rx }
    }

    /// Connect to MSIM on the specified port and set up the background thread for message handling.
    pub fn connect(port: u16, event_tx: DebugEventSender) -> Result<Self> {
        let stream = connect(port)?;
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();

        post_msim_background(stream.try_clone()?, event_tx, resp_tx);
        Ok(Self::new(stream, resp_rx))
    }
}

fn post_msim_background(
    mut msim_stream: TcpStream,
    event_tx: DebugEventSender,
    resp_tx: ResponseTx,
) {
    std::thread::spawn(move || {
        loop {
            match Inbound::read(&mut msim_stream) {
                Ok(inbound) => match inbound {
                    // Responses go to separate internal channel
                    Inbound::Response {
                        status,
                        arg0,
                        arg1,
                        arg2,
                    } => {
                        // If the receiver is dropped, just exit the thread
                        if resp_tx
                            .send(RawResponse {
                                status,
                                arg0,
                                arg1,
                                arg2,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }

                    // Events go to the main event channel
                    Inbound::Event {
                        kind,
                        arg0,
                        arg1,
                        arg2,
                    } => {
                        let (msg, fatal) = match Event::from_raw(kind, arg0, arg1, arg2) {
                            Ok(event) => (Ok(DebugEvent::MsimEvent(event)), false),
                            Err(e) => (Err(MsimError::from(e).into()), true),
                        };
                        if event_tx.send(msg).is_err() || fatal {
                            break;
                        }
                    }
                },

                // EOF means the connection was closed, just exit
                Err(FrameError::IO(io_e)) if io_e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }

                Err(e) => {
                    event_tx.send(Err(MsimError::from(e).into())).ok();
                    break;
                }
            }
        }
    });
}

impl RawResponse {
    // TODO(!!!): serious rework, this should somehow only return the args if ok - think it through
    pub const fn to_result(self) -> Result<Self> {
        match self.status {
            ResponseStatus::Ok => Ok(self),
            _ => Err(MsimError::RequestFailed), // Any error status
        }
    }
}

impl Connection for TcpConnection {
    fn send(&mut self, request: Request) -> Result<RawResponse> {
        if self.resp_rx.try_recv().is_ok() {
            return Err(MsimError::UnexpectedMessage);
        }

        request.write(&mut self.stream)?;

        self.resp_rx.recv().map_err(|_| MsimError::ListenerDied)
    }
}

impl From<FrameError> for MsimError {
    fn from(e: FrameError) -> Self {
        match e {
            FrameError::IO(io_e) => io_e.into(),
            FrameError::Parsing => Self::ParseError,
        }
    }
}
