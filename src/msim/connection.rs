use super::Result;
use crate::msim::commands::{MsimRequest, MsimResponse};
use crate::msim::message::RequestMessage;
use crate::msim::tcp::connect;
use std::io::ErrorKind;
use std::net::TcpStream;

pub trait MsimConnection {
    fn send(&mut self, command: MsimRequest) -> Result<MsimResponse>;
}

pub struct TcpMsimConnection {
    stream: TcpStream,
}

impl TcpMsimConnection {
    pub fn new(port: u16) -> Result<Self> {
        Ok(TcpMsimConnection {
            stream: connect(port)?,
        })
    }
}

impl MsimConnection for TcpMsimConnection {
    fn send(&mut self, request: MsimRequest) -> Result<MsimResponse> {
        let message: RequestMessage = request.into();
        message.write(&mut self.stream)?;

        // TODO: add support in MSIM and remove this
        Err(
            std::io::Error::new(ErrorKind::Unsupported, "Response not yet supported in MSIM")
                .into(),
        )
        // Ok(ResponseMessage::read(&mut self.stream)?.into())
    }
}
