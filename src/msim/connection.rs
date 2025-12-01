use super::Result;
use crate::msim::commands::{MsimCommand, MsimResponse};
use crate::msim::message::{Message, MessageType};
use crate::msim::tcp::connect;
use std::net::TcpStream;

pub trait MsimConnection {
    fn send_command(&mut self, command: MsimCommand) -> Result<MsimResponse>;
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

    fn send_command(&mut self, command: MsimCommand) -> Result<MsimResponse> {
        let address = match command {
            MsimCommand::SetBreakpoint(address) => address,
            _ => 0
        };

        let message = Message {
            msg_type: MessageType::Breakpoint,
            address
        };

        message.write(&mut self.stream)?;

        Ok(Message::read(&mut self.stream)?.try_into()?)
    }
}
