use super::Result;
use super::message::{Message, MessageType};
use super::tcp::connect;
use std::net::TcpStream;

pub trait Commands {
    fn set_breakpoint(&mut self, address: u32) -> Result<()>;
}

/// MSIM network commander
pub struct Commander {
    stream: TcpStream,
}

impl Commander {
    pub fn new(port: u16) -> Result<Self> {
        Ok(Commander {
            stream: connect(port)?,
        })
    }
}

impl Commands for Commander {
    fn set_breakpoint(&mut self, address: u32) -> Result<()> {
        let message = Message {
            msg_type: MessageType::Breakpoint,
            address,
        };

        message.write(&mut self.stream)
    }
}
