use super::Result;
use crate::msim::MSIMError;
use crate::msim::commands::MsimCommand::SetBreakpoint;
use crate::msim::commands::{MsimCommand, MsimResponse};
use crate::msim::message::{Message, MessageType};
use std::io::ErrorKind;

impl From<MsimCommand> for Message {
    fn from(command: MsimCommand) -> Self {
        match command {
            SetBreakpoint(address) => Message {
                msg_type: MessageType::Breakpoint,
                address,
            },
        }
    }
}

impl TryFrom<Message> for MsimResponse {
    type Error = MSIMError;

    fn try_from(message: Message) -> Result<Self> {
        match message.msg_type {
            MessageType::NoOp | MessageType::Breakpoint => {
                Err(std::io::Error::new(ErrorKind::InvalidData, "bad message type").into())
            }
            MessageType::StoppedAt => Ok(MsimResponse::Stopped(message.address))
        }
    }
}
