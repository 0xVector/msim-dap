use super::message::{Inbound, MessageError, Request, ResponseKind};
use super::tcp::connect;
use super::{MSIMError, RequestError, Result};
use crate::{DebugEvent, DebugEventSender};
use std::net::TcpStream;

pub trait Connection {
    fn send(&mut self, command: Request) -> Result<()>;
}

type ResponseTx = std::sync::mpsc::Sender<ResponseKind>;
type ResponseRx = std::sync::mpsc::Receiver<ResponseKind>;

pub struct TcpConnection {
    stream: TcpStream,
    resp_rx: ResponseRx,
}

impl TcpConnection {
    fn new(stream: TcpStream, resp_rx: ResponseRx) -> Self {
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
                    // Responses go to separate channel
                    Inbound::Response(resp) => {
                        if resp_tx.send(resp).is_err() {
                            break;
                        }
                    }
                    Inbound::Event(event) => {
                        if event_tx.send(Ok(DebugEvent::MsimEvent(event))).is_err() {
                            break;
                        }
                    }
                },

                // EOF means the connection was closed, just exit
                Err(MessageError::IOError(io_e))
                    if io_e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }

                Err(e) => {
                    event_tx
                        .send(Err(MSIMError::from(e).into()))
                        .ok();
                    break;
                }
            }
        }
    });
}

impl Connection for TcpConnection {
    fn send(&mut self, request: Request) -> Result<()> {
        if self.resp_rx.try_recv().is_ok() {
            return Err(MSIMError::UnexpectedMessage);
        }

        request.write(&mut self.stream)?;

        match self.resp_rx.recv() {
            Ok(resp) => resp.into_result(),

            // Channel closed
            Err(_) => Err(MSIMError::ListenerDied),
        }
    }
}

impl From<MessageError> for MSIMError {
    fn from(e: MessageError) -> Self {
        match e {
            MessageError::IOError(io_e) => io_e.into(),
            MessageError::ProtocolError => MSIMError::ParseError,
        }
    }
}

impl ResponseKind {
    pub fn into_result(self) -> Result<()> {
        match self {
            ResponseKind::Ok => Ok(()),
            ResponseKind::UnspecifiedError => {
                Err(MSIMError::RequestFailed(RequestError::UnspecifiedError))
            }
        }
    }
}
