use crate::Address;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

pub type Result<T> = std::result::Result<T, MessageError>;

#[derive(thiserror::Error, Debug)]
pub enum MessageError {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    /// Error while parsing the MSIM protocol
    #[error("Protocol error")]
    ProtocolError,
}

/// Types of requests that can be sent to MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    SetBreakpoint(Address) = 0x01,
    Resume = 0x02,
}

/// Types of inbound messages from MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Inbound {
    Response(ResponseKind) = 0x00,
    Event(EventKind) = 0x01,
}

/// Types of responses that can be received from MSIM in response to a request.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ResponseKind {
    /// Response indicating that the request was successful.
    /// Represented on the wire as a single byte with value 0.
    #[default]
    Ok = 0x00,
    /// Response indicating that the request failed with an unspecified error.
    UnspecifiedError = 0x01,
}

/// Types of events that can be received from MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    StoppedAt(Address) = 0x01,
}

impl Request {
    /// Write the Request to the given writer.
    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        let (kind, address) = match self {
            Self::SetBreakpoint(a) => (0x01, *a),
            Self::Resume => (0x02, 0),
        };
        writer.write_u8(kind)?;
        writer.write_u32::<BigEndian>(address)?;

        Ok(())
    }
}

impl Inbound {
    /// Read an Inbound message from the given reader.
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        match reader.read_u8()? {
            0x00 => Ok(Self::Response(ResponseKind::read(reader)?)),
            0x01 => Ok(Self::Event(EventKind::read(reader)?)),
            _ => Err(MessageError::ProtocolError),
        }
    }
}
impl ResponseKind {
    /// Read a ResponseKind from the given reader.
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        match reader.read_u8()? {
            0x00 => Ok(Self::Ok),
            0x01 => Ok(Self::UnspecifiedError),
            _ => Err(MessageError::ProtocolError),
        }
    }
}

impl EventKind {
    /// Read an EventKind from the given reader.
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        match reader.read_u8()? {
            0x01 => Ok(Self::StoppedAt(reader.read_u32::<BigEndian>()?)),
            _ => Err(MessageError::ProtocolError),
        }
    }
}
