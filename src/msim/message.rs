use super::Result;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

/// Types of requests that can be sent to MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RequestType {
    #[default]
    NoOp = 0,
    SetBreakpoint = 1,
    Continue = 2,
}

/// Types of responses that can be received from MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseType {
    Ok = 0,
    StoppedAt = 1,
}

/// On the wire message format for requests to MSIM.
/// Consists of a message type and an address.
/// The address is only meaningful for certain message types.
#[derive(Debug, Default)]
pub struct RequestMessage {
    pub msg_type: RequestType,
    pub address: u32,
}

impl RequestMessage {
    /// Writes the RequestMessage to the given writer.
    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_u8(self.msg_type as u8)?;
        writer.write_u32::<BigEndian>(self.address)?;

        Ok(())
    }
}

/// On the wire message format for responses from MSIM.
/// Consists of a message type and an address.
/// The address is only meaningful for certain message types.
#[derive(Debug)]
pub struct ResponseMessage {
    pub msg_type: ResponseType,
    pub address: u32,
}

impl ResponseMessage {
    /// Reads a ResponseMessage from the given reader.
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        let msg_type = reader.read_u8()?;
        let address = reader.read_u32::<BigEndian>()?;

        let msg_type = match msg_type {
            0 => ResponseType::Ok,
            1 => ResponseType::StoppedAt,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "bad message type").into()),
        };

        Ok(ResponseMessage { msg_type, address })
    }
}
