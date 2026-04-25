use super::frame::{FrameError, Inbound, Request, ResponseStatus};
use super::tcp::connect;
use super::{Event, MsimError, Result};
use crate::{DebugEvent, DebugEventSender};
use std::net::TcpStream;

pub trait Connection {
    fn send(&mut self, command: Request) -> Result<RawResponse>;
}

#[allow(unused)] // TODO: implement & use
pub struct RawResponse {
    pub status: ResponseStatus,
    pub arg0: u64,
    pub arg1: u64,
}

type ResponseTx = std::sync::mpsc::Sender<RawResponse>;
type ResponseRx = std::sync::mpsc::Receiver<RawResponse>;

pub struct TcpConnection {
    stream: TcpStream,
    resp_rx: ResponseRx, // Internal channel for receiving responses from the background thread.
}

impl TcpConnection {
    const fn new(stream: TcpStream, resp_rx: ResponseRx) -> Self {
        Self { stream, resp_rx }
    }

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
                    Inbound::Response { status, arg0, arg1 } => {
                        // If the receiver is dropped, just exit the thread
                        if resp_tx.send(RawResponse { status, arg0, arg1 }).is_err() {
                            break;
                        }
                    }

                    // Events go to the main event channel
                    Inbound::Event { kind, arg0, arg1 } => {
                        let (msg, fatal) = match Event::from_raw(kind, arg0, arg1) {
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
    pub const fn get_result(&self) -> Result<()> {
        match self.status {
            ResponseStatus::Ok => Ok(()),
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
