use super::{MSIMError, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write, read_to_string};

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MessageType {
    NoOp = 0,
    Breakpoint = 1,
    StoppedAt = 2,
}

#[derive(Debug)]
pub struct Message {
    pub msg_type: MessageType,
    pub address: u32,
}

impl Message {
    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_u8(self.msg_type as u8)?;
        writer.write_u32::<BigEndian>(self.address)?;

        Ok(())
    }

    pub fn read(reader: &mut impl Read) -> Result<Self> {
        let mut header = [0u8; 2];

        reader.read_exact(&mut header)?;
        let address = reader.read_u32::<BigEndian>()?;

        let msg_type = match header[0] {
            0 => MessageType::NoOp,
            1 => MessageType::Breakpoint,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "bad message type").into()),
        };

        Ok(Message { msg_type, address })
    }
}
