use crate::{Address, LineNo};
use std::path::Path;

mod msim_target;

pub use msim_target::MsimTarget;

pub type Result<T> = std::result::Result<T, TargetError>;

#[derive(thiserror::Error, Debug)]
pub enum TargetError {
    /// Fatal error
    #[error("Target session lost")]
    SessionLost,

    #[error("Request failed")]
    RequestFailed,

    // TODO: are the params even needed?
    #[error("Address not found for {0}:{1}")]
    AddressNotFound(String, LineNo),

    #[error("Address {0} is out of range")]
    AddressOutOfRange(Address),
}

pub trait DebugTarget {
    /// Resume execution of the target.
    fn resume(&mut self) -> Result<()>;

    /// Pause execution of the target.
    fn pause(&mut self) -> Result<()>;

    /// Stop the target.
    fn stop(&mut self) -> Result<()>;

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
}
