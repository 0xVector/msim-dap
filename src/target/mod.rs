use crate::msim::{CsrAddress, RegisterId};
use crate::{Address, CpuId, LineNo};
use std::path::Path;

mod msim_target;
#[cfg(test)]
mod tests;

pub use msim_target::MsimTarget;

pub type Result<T> = std::result::Result<T, TargetError>;

#[derive(thiserror::Error, Debug)]
pub enum TargetError {
    /// Fatal error
    #[error("Target session lost")]
    SessionLost,

    // Rest are recoverable errors that can be handled by the debugger
    #[error("Request failed in Target")]
    RequestFailed,

    // TODO: are the params even needed?
    #[error("Address not found for {0}:{1}")]
    AddressNotFound(String, LineNo),

    #[error("Address {0} is out of range")]
    AddressOutOfRange(Address),

    #[error("CPU {0} not found")]
    UnknownCpu(CpuId),

    #[error("Unknown architecture")]
    UnknownArch,

    #[error("General-purpose register {0:#x} not found")]
    BadGeneralReg(RegisterId),

    #[error("CSR register {0:#x} not found")]
    BadCsrReg(CsrAddress),

    #[error("General-purpose register {0} not found")]
    UnknownRegisterName(String),

    #[error("CSR {0} not found")]
    UnknownCsrName(String),
}

pub struct Register {
    pub name: &'static str,
    pub value: u64,
}

pub trait DebugTarget {
    /// Get the number of CPUs in the target.
    fn cpu_count(&mut self) -> Result<u64>;

    /// Resume execution of the target.
    fn resume(&mut self) -> Result<()>;

    /// Pause execution of the target.
    fn pause(&mut self) -> Result<()>;

    /// Terminate the target.
    fn terminate(&mut self) -> Result<()>;

    /// Step the given CPU by a given number of instructions
    fn step_by(&mut self, cpu: CpuId, count: u64) -> Result<()>;

    /// Set a code breakpoint at the given source and line, returning the address of the breakpoint.
    /// Does not affect existing breakpoints.
    /// If a breakpoint already exists at the given location, it does not set a new one.
    fn set_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address>;

    /// Remove a code breakpoint at the given source and line, returning the address of the removed breakpoint.
    fn remove_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address>;

    /// Replace all code BPs for the given source with the given lines.
    /// Returns a result for each line, but does not fail fatally if some lines fail.
    fn replace_code_bps(&mut self, source: &Path, lines: &[LineNo]) -> Vec<Result<()>>;

    /// Resolve the address to a source and line if it corresponds to a code BP, otherwise None.
    fn resolve_code_bp(&self, address: Address) -> Option<(&Path, LineNo)>;

    /// Resolve the given address to a source and line.
    fn resolve_address(&self, address: Address) -> Option<(&Path, LineNo)>;

    /// Read the value of all general-purpose registers on the given CPU.
    fn read_general_regs(&mut self, cpu: CpuId) -> Result<Vec<Register>>;

    /// Write the value of the general-purpose register with the given name on the given CPU.
    fn write_general_reg(&mut self, cpu: CpuId, name: &str, value: u64) -> Result<()>;

    /// Read the value of all CSR registers on the given CPU.
    fn read_csrs(&mut self, cpu: CpuId) -> Result<Vec<Register>>;

    /// Write the value of the CSR register (or program counter) with the given name on the given CPU.
    fn write_csr(&mut self, cpu: CpuId, name: &str, value: u64) -> Result<()>;

    /// Read the program counter of the given CPU.
    fn read_pc(&mut self, cpu: CpuId) -> Result<Address>;

    #[allow(unused)]
    /// Write the program counter of the given CPU.
    fn write_pc(&mut self, cpu: CpuId, address: Address) -> Result<()>;

    /// Read `length` bytes of memory starting at the given physical address.
    fn read_phys_memory(&mut self, address: Address, length: usize) -> Result<Vec<u8>>;

    /// Read `length` bytes of memory starting at the given virtual address.
    fn read_virt_memory(&mut self, cpu: CpuId, address: Address, length: usize) -> Result<Vec<u8>>;
}
