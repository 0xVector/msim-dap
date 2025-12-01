use super::Result;
use crate::msim::commands::{MsimRequest, MsimResponse};
use crate::msim::message::{RequestMessage, RequestType, ResponseMessage};
use crate::msim::tcp::connect;
use std::net::TcpStream;

pub trait MsimConnection {
    fn send_command(&mut self, command: MsimRequest) -> Result<MsimResponse>;
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
    fn send_command(&mut self, command: MsimRequest) -> Result<MsimResponse> {
        let address = match command {
            MsimRequest::SetBreakpoint(address) => address,
            _ => 0,
        };

        let message = RequestMessage {
            msg_type: RequestType::SetBreakpoint,
            address,
        };

        message.write(&mut self.stream)?;

        Ok(ResponseMessage::read(&mut self.stream)?.into())
    }
}
