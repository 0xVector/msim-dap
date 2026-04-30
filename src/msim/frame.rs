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
/// Always `25 B`: `1 B` for the request kind and `24 B` for the three arguments (`arg0`, `arg1` and `arg2`).
pub const OUTBOUND_FRAME_SIZE: usize = 25;
const _: () = assert!(
    OUTBOUND_FRAME_SIZE == size_of::<u8>() + 3 * size_of::<ArgType>(),
    "Outbound frame size must be 20 bytes"
);

/// Size of an inbound frame (response or event) in bytes.
/// Always `26 B`: `1 B` for the category, `1 B` for the response/event kind, and `24 B` for the three arguments (`arg0`, `arg1` and `arg2`).
pub const INBOUND_FRAME_SIZE: usize = 26;
const _: () = assert!(
    INBOUND_FRAME_SIZE == size_of::<u8>() + size_of::<u8>() + 3 * size_of::<ArgType>(),
    "Inbound frame size must be 26 bytes"
);

// Field types
pub type InstructionCount = u64;
pub type CpuId = u64;
pub type RegisterId = u64;
pub type CsrId = u64;
pub type InterruptId = u64;

// Types used in the MSIM protocol

/// Data to write to memory, always `8 B`.
pub type MemoryWriteData = u64;
/// Generic argument type used in all requests, responses and events (arg0, arg1 and arg2). Always `8 B`.
pub type ArgType = u64;
/// Type of the request kind field. Always `1 B`.
pub type RequestType = u8;
/// Type of the response status field. Always `1 B`.
pub type StatusType = u8;
/// Type of the event kind field. Always `1 B`.
pub type EventKindType = u8;

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

    /// Request to set a physical data breakpoint at `arg0=address` with `arg1=kind`. `0x07`
    SetPhysDataBreakpoint {
        address: Address,
        kind: DataBreakpointKind,
    } = 0x07,
    /// Request to remove a physical data breakpoint at `arg0=address`. `0x08`
    RemovePhysDataBreakpoint(Address) = 0x08,

    // Register requests
    /// Request to read the value of register `arg1=reg_id` from CPU `arg0=cpu_id`. `0x09`
    ReadGeneralRegister { cpu: CpuId, reg: RegisterId } = 0x09,
    /// Request to write to register `arg1=reg_id` of CPU `arg0=cpu_id` the value `arg2=value`. `0x0A`
    WriteGeneralRegister {
        cpu: CpuId,
        reg: RegisterId,
        value: ArgType,
    } = 0x0A,

    /// Request to read the value of Control and Status Register (CSR) `arg1=id` from CPU `arg0=cpu_id`. `0x0B`
    ReadCsr { cpu: CpuId, reg: CsrId } = 0x0B,
    /// Request to write to Control and Status Register (CSR) `arg1=id` of CPU `arg0=cpu_id` the value `arg2=value`. `0x0C`
    WriteCsr {
        cpu: CpuId,
        reg: CsrId,
        value: ArgType,
    } = 0x0C,

    /// Request to read the value of the program counter from CPU `arg0=cpu_id`. `0x0D`
    ReadPC(CpuId) = 0x0D,
    /// Request to write `arg1=value` to the program counter of CPU `arg0=cpu_id`. `0x0E`
    WritePC { cpu: CpuId, value: Address } = 0x0E,

    // Memory requests
    /// Request to read physical memory at `arg0=address`. `0x0F`
    ReadPhysMemory(Address) = 0x0F,
    /// Request to write `arg1=data` (8 B) to physical memory at `arg0=address`. `0x10`
    WritePhysMemory {
        address: Address,
        data: MemoryWriteData,
    } = 0x10,

    /// Request to read virtual memory in the context of `arg0=cpu_id` at `arg1=address`. `0x11`
    ReadVirtMemory { cpu: CpuId, address: Address } = 0x11,
    /// Request to write `arg2=data` (8 B) to virtual memory in the context of `arg0=cpu_id` at `arg1=address`. `0x12`
    WriteVirtMemory {
        cpu: CpuId,
        address: Address,
        data: MemoryWriteData,
    } = 0x12,

    /// Request to translate in the context of `arg0=cpu_id` the virtual address `arg1=address` to a physical address. `0x13`
    TranslateAddress { cpu: CpuId, address: Address } = 0x13,

    // Interrupt requests
    /// Request to raise interrupt `arg1=id` on `arg0=cpu_id`. `0x14`
    RaiseInterrupt { cpu: CpuId, id: InterruptId } = 0x14,
    /// Request to clear interrupt `arg1=id` on `arg0=cpu_id`. `0x15`
    ClearInterrupt { cpu: CpuId, id: InterruptId } = 0x15,

    /// Request to get the current MSIM configuration. `0x16`
    GetConfig = 0x16,

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
/// [Category (1 B)] [(Response|Event)Kind (1 B)] [arg0 (8 B, BE)] [arg1 (8 B, BE)] [arg2 (8 B, BE)]
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
        arg2: ArgType,
    } = 0x01,

    /// MSIM event notification, with [`EventKind`] and optional arguments. `0x02`
    Event {
        kind: EventKind,
        arg0: ArgType,
        arg1: ArgType,
        arg2: ArgType,
    } = 0x02,
}

/// Status of a response from MSIM, used in the [`Inbound::Response`] variant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStatus {
    /// 0x00 reserved for uninitialized responses, should not be received on the wire.

    /// Response indicating that the request was successful. `0x01`
    Ok = 0x01,

    /// Response indicating that the request failed with an unspecified error. `0x02`
    UnspecifiedError = 0x02,

    /// Response indicating that the request is not supported by this MSIM version. `0x03`
    UnsupportedRequestError = 0x03,
}

// TODO: more event kinds
/// Kinds of events that can be received from MSIM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// 0x00 Reserved for uninitialized events, should not be received on the wire.

    /// Event indicating that the simulator has exited. `0x01`
    Exited = 0x01,

    /// Event indicating that the simulator has stopped at `arg0=address` due to `arg1=reason`. `0x02`
    StoppedAt = 0x02,
}

/// Reasons for stopping at an address, used in the [`EventKind::StoppedAt`] variant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoppedAtReason {
    /// Reason indicating that the simulator is paused due to a pause request. `0x01`
    Paused = 0x01,
    /// Reason indicating that the simulator is paused due to hitting a breakpoint. `0x02`
    Breakpoint = 0x02,
    /// Reason indicating that the simulator is paused due to completing a step request. `0x03`
    StepComplete = 0x03,
    /// Reason indicating that the simulator is paused due to an interrupt. `0x04`
    Interrupt = 0x04,
}

impl Request {
    /// Write the Request to the given writer.
    pub fn write(self, writer: &mut impl Write) -> Result<()> {
        let (kind, arg0, arg1, arg2): (RequestType, ArgType, ArgType, ArgType) = match self {
            Self::Resume => (0x01, 0x00, 0x00, 0x00),
            Self::Pause => (0x02, 0x00, 0x00, 0x00),
            Self::Stop => (0x03, 0x00, 0x00, 0x00),
            Self::Step(count) => (0x04, count, 0x00, 0x00),
            Self::SetCodeBreakpoint(address) => (0x05, address, 0x00, 0x00),
            Self::RemoveCodeBreakpoint(address) => (0x06, address, 0x00, 0x00),
            Self::SetPhysDataBreakpoint { address, kind } => (0x07, address, kind as u64, 0x00),
            Self::RemovePhysDataBreakpoint(address) => (0x08, address, 0x00, 0x00),
            Self::ReadGeneralRegister { cpu, reg } => (0x09, cpu, reg, 0x00),
            Self::WriteGeneralRegister { cpu, reg, value } => (0x0A, cpu, reg, value),
            Self::ReadCsr { cpu, reg } => (0x0B, cpu, reg, 0x00),
            Self::WriteCsr { cpu, reg, value } => (0x0C, cpu, reg, value),
            Self::ReadPC(cpu) => (0x0D, cpu, 0x00, 0x00),
            Self::WritePC { cpu, value } => (0x0E, cpu, value, 0x00),
            Self::ReadPhysMemory(address) => (0x0F, address, 0x00, 0x00),
            Self::WritePhysMemory { address, data } => (0x10, address, data, 0x00),
            Self::ReadVirtMemory { cpu, address } => (0x11, cpu, address, 0x00),
            Self::WriteVirtMemory { cpu, address, data } => (0x12, cpu, address, data),
            Self::TranslateAddress { cpu, address } => (0x13, cpu, address, 0x00),
            Self::RaiseInterrupt { cpu, id } => (0x14, cpu, id, 0x00),
            Self::ClearInterrupt { cpu, id } => (0x15, cpu, id, 0x00),
            Self::GetConfig => (0x16, 0x00, 0x00, 0x00),
        };
        writer.write_u8(kind)?;
        writer.write_u64::<BigEndian>(arg0)?;
        writer.write_u64::<BigEndian>(arg1)?;
        writer.write_u64::<BigEndian>(arg2)?;
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
        let arg2 = reader.read_u64::<BigEndian>()?;

        match category {
            0x01 => Ok(Self::Response {
                status: ResponseStatus::read(kind)?,
                arg0,
                arg1,
                arg2,
            }),
            0x02 => Ok(Self::Event {
                kind: EventKind::read(kind)?,
                arg0,
                arg1,
                arg2,
            }),
            _ => Err(FrameError::Parsing),
        }
    }
}
impl ResponseStatus {
    /// Read a [`ResponseStatus`] from the given reader.
    pub const fn read(status: StatusType) -> Result<Self> {
        match status {
            0x01 => Ok(Self::Ok),
            0x02 => Ok(Self::UnspecifiedError),
            0x03 => Ok(Self::UnsupportedRequestError),
            _ => Err(FrameError::Parsing),
        }
    }
}

impl EventKind {
    /// Read an [`EventKind`] from the given input.
    pub const fn read(kind: EventKindType) -> Result<Self> {
        match kind {
            0x01 => Ok(Self::Exited),
            0x02 => Ok(Self::StoppedAt),
            _ => Err(FrameError::Parsing),
        }
    }
}

impl StoppedAtReason {
    /// Read a [`StoppedAtReason`] from the given input.
    pub const fn read(reason: ArgType) -> Result<Self> {
        match reason {
            0x01 => Ok(Self::Paused),
            0x02 => Ok(Self::Breakpoint),
            0x03 => Ok(Self::StepComplete),
            0x04 => Ok(Self::Interrupt),
            _ => Err(FrameError::Parsing),
        }
    }
}
