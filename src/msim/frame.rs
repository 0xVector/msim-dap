use crate::Address;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

pub type Result<T> = std::result::Result<T, FrameError>;

/// Low-level frame error type
#[derive(thiserror::Error, Debug)]
pub enum FrameError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    /// Error while parsing the MSIM protocol
    #[error("Parsing error")]
    Parsing,
}

/// Size of an outbound frame (request) in bytes.
/// Always `17 B`: `1 B` for the request kind and `16 B` for the two arguments (`arg0` and `arg1`).
pub const OUTBOUND_FRAME_SIZE: usize = 17;
const _: () = assert!(
    OUTBOUND_FRAME_SIZE == size_of::<u8>() + 2 * size_of::<ArgType>(),
    "Outbound frame size must be 17 bytes"
);

/// Size of an inbound frame (response or event) in bytes.
/// Always `18 B`: `1 B` for the category, `1 B` for the response/event kind, and `16 B` for the two arguments (`arg0` and `arg1`).
pub const INBOUND_FRAME_SIZE: usize = 18;
const _: () = assert!(
    INBOUND_FRAME_SIZE == size_of::<u8>() + size_of::<u8>() + 2 * size_of::<ArgType>(),
    "Inbound frame size must be 18 bytes"
);

// Field types
pub type InstructionCount = u64;
pub type RegisterId = u64;
pub type CsrId = u64;
pub type InterruptId = u64;

// Types used in the MSIM protocol

/// Data to write to memory, always `8 B`.
pub type MemoryWriteData = u64;
/// Generic argument type used in all requests, responses and events (arg0 and arg1). Always `8 B`.
pub type ArgType = u64;

/// Types of requests that can be sent to MSIM.
/// The request type is determined by the first byte of the frame and is the same as the rust
/// enum variant discriminant.
///
/// The request can have additional arguments.
/// The format is:  
/// ```text
/// [RequestKind (1 B)] [arg0 (8 B, BE)] [arg1 (8 B, BE)]
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)] // TODO: implement & use
pub enum Request {
    // 0x00 reserved for uninitialized requests, should not be sent on the wire.

    // Control flow requests
    /// Request to resume execution. Also used for the initial start. `0x01`
    Resume = 0x01,
    /// Request to pause execution. `0x02`
    Pause = 0x02,
    /// Request to stop execution and exit the simulator. `0x03`
    Stop = 0x03,
    /// Request to step `arg0=count` instructions. `0x04`
    Step(InstructionCount) = 0x04,

    // Breakpoint requests
    /// Request to set a code breakpoint at `arg0=address`. `0x05`
    SetCodeBreakpoint(Address) = 0x05,
    /// Request to remove a code breakpoint at `arg0=address`. `0x06`
    RemoveCodeBreakpoint(Address) = 0x06,

    /// Request to set a data breakpoint at `arg0=address` with `arg1=kind`. `0x07`
    SetDataBreakpoint {
        address: Address,
        kind: DataBreakpointKind,
    } = 0x07,
    /// Request to remove a data breakpoint at `arg0=address`. `0x08`
    RemoveDataBreakpoint(Address) = 0x08,

    // Register requests
    /// Request to read the value of register `arg0=id`. `0x09`
    ReadGeneralRegister(RegisterId) = 0x09,
    /// Request to write to register `arg0=id` the value `arg1=value`. `0x0A`
    WriteGeneralRegister { id: RegisterId, value: u64 } = 0x0A,

    /// Request to read the value of Control and Status Register (CSR) `arg0=id`. `0x0B`
    ReadCsr(CsrId) = 0x0B,
    /// Request to write to Control and Status Register (CSR) `arg0=id` the value `arg1=value`. `0x0C`
    WriteCsr { id: CsrId, value: u64 } = 0x0C,

    /// Request to read the value of the program counter. `0x0D`
    ReadPC = 0x0D,
    /// Request to write `arg0=value` to the program counter. `0x0E`
    WritePC(Address) = 0x0E,

    // Memory requests
    /// Request to read physical memory at `arg0=address`. `0x0F`
    ReadPhysMemory(Address) = 0x0F,
    /// Request to write `arg1=data` (8 B) to physical memory at `arg0=address`. `0x10`
    WritePhysMemory {
        address: Address,
        data: MemoryWriteData,
    } = 0x10,

    /// Request to read virtual memory at `arg0=address`. `0x11`
    ReadVirtMemory(Address) = 0x11,
    /// Request to write `arg1=data` (8 B) to virtual memory at `arg0=address`. `0x12`
    WriteVirtMemory {
        address: Address,
        data: MemoryWriteData,
    } = 0x12,

    /// Request to translate the virtual address `arg0=address` to a physical address. `0x13`
    TranslateAddress(Address) = 0x13,

    // Interrupt requests
    /// Request to raise interrupt `arg0=id`. `0x14`
    RaiseInterrupt(InterruptId) = 0x14,
    /// Request to clear interrupt `arg0=id`. `0x15`
    ClearInterrupt(InterruptId) = 0x15,
    //
    // TODO: GetTlbEntry(index)
}

/// Kind of data breakpoint, used in the [`DataBreakpoint`] struct.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)] // TODO: implement & use
pub enum DataBreakpointKind {
    Read = 0x01,
    Write = 0x02,
    ReadWrite = 0x03,
}

/// Categories of inbound frames from MSIM.
///
/// Format:  
/// ```text
/// [Category (1 B)] [(Response|Event)Kind (1 B)] [arg0 (8 B, BE)] [arg1 (8 B, BE)]
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound {
    /// 0x00 reserved for uninitialized frames, should not be received on the wire.

    /// Response to a request, with [`ResponseStatus`] and optional arguments. `0x01`
    Response {
        status: ResponseStatus,
        arg0: ArgType,
        arg1: ArgType,
    } = 0x01,

    /// MSIM event notification, with [`EventKind`] and optional arguments. `0x02`
    Event {
        kind: EventKind,
        arg0: ArgType,
        arg1: ArgType,
    } = 0x02,
}

/// Status of a response from MSIM, used in the [`Inbound::Response`] variant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStatus {
    /// 0x00 reserved for uninitialized responses, should not be received on the wire.

    /// Response indicating that the request was successful. `0x01`
    Ok = 0x01,

    /// Response indicating that the request failed with an error. `0x02`
    Error = 0x02,
}

/// Kinds of events that can be received from MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// 0x00 Reserved for uninitialized events, should not be received on the wire.

    /// Event indicating that the simulator has exited. `0x01`
    Exited = 0x01,

    // TODO: add reason to differentiate BPs etc
    /// Event indicating that the simulator has stopped at `arg0=address`. `0x02`
    StoppedAt = 0x02,
}

impl Request {
    /// Write the Request to the given writer.
    pub fn write(self, writer: &mut impl Write) -> Result<()> {
        let (kind, arg0, arg1): (u8, u64, u64) = match self {
            Self::Resume => (0x01, 0x00, 0x00),
            Self::Pause => (0x02, 0x00, 0x00),
            Self::Stop => (0x03, 0x00, 0x00),
            Self::Step(count) => (0x04, count, 0x00),
            Self::SetCodeBreakpoint(address) => (0x05, address, 0x00),
            Self::RemoveCodeBreakpoint(address) => (0x06, address, 0x00),
            Self::SetDataBreakpoint { address, kind } => (0x07, address, kind as u64),
            Self::RemoveDataBreakpoint(address) => (0x08, address, 0x00),
            Self::ReadGeneralRegister(id) => (0x09, id, 0x00),
            Self::WriteGeneralRegister { id, value } => (0x0A, id, value),
            Self::ReadCsr(id) => (0x0B, id, 0x00),
            Self::WriteCsr { id, value } => (0x0C, id, value),
            Self::ReadPC => (0x0D, 0x00, 0x00),
            Self::WritePC(address) => (0x0E, address, 0x00),
            Self::ReadPhysMemory(address) => (0x0F, address, 0x00),
            Self::WritePhysMemory { address, data } => (0x10, address, data),
            Self::ReadVirtMemory(address) => (0x11, address, 0x00),
            Self::WriteVirtMemory { address, data } => (0x12, address, data),
            Self::TranslateAddress(address) => (0x13, address, 0x00),
            Self::RaiseInterrupt(id) => (0x14, id, 0x00),
            Self::ClearInterrupt(id) => (0x15, id, 0x00),
        };
        writer.write_u8(kind)?;
        writer.write_u64::<BigEndian>(arg0)?;
        writer.write_u64::<BigEndian>(arg1)?;
        Ok(())
    }
}

impl Inbound {
    /// Read an Inbound frame from the given reader.
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        let category = reader.read_u8()?;
        let kind = reader.read_u8()?;
        let arg0 = reader.read_u64::<BigEndian>()?;
        let arg1 = reader.read_u64::<BigEndian>()?;

        match category {
            0x01 => Ok(Self::Response {
                status: ResponseStatus::read(kind)?,
                arg0,
                arg1,
            }),
            0x02 => Ok(Self::Event {
                kind: EventKind::read(kind)?,
                arg0,
                arg1,
            }),
            _ => Err(FrameError::Parsing),
        }
    }
}
impl ResponseStatus {
    /// Read a [`ResponseStatus`] from the given reader.
    pub const fn read(status: u8) -> Result<Self> {
        match status {
            0x01 => Ok(Self::Ok),
            0x02 => Ok(Self::Error),
            _ => Err(FrameError::Parsing),
        }
    }
}

impl EventKind {
    /// Read an [`EventKind`] from the given reader.
    pub const fn read(kind: u8) -> Result<Self> {
        match kind {
            0x01 => Ok(Self::Exited),
            0x02 => Ok(Self::StoppedAt),
            _ => Err(FrameError::Parsing),
        }
    }
}
