use super::{DebugTarget, Result, TargetError};
use crate::dwarf::DwarfIndex;
use crate::msim::{Connection, MSIMError, Request};
use std::path::Path;

pub struct MsimTarget<S: Connection> {
    connection: S,
    index: DwarfIndex,
}

impl<S: Connection> MsimTarget<S> {
    pub const fn new(session: S, index: DwarfIndex) -> Self {
        Self {
            connection: session,
            index,
        }
    }
}

impl<S: Connection> DebugTarget for MsimTarget<S> {
    fn resume(&mut self) -> Result<()> {
        Ok(self.connection.send(Request::Resume)?)
    }

    fn set_breakpoint(&mut self, source: &Path, line: u64) -> Result<()> {
        let address = self
            .index
            .get_address(source, line)
            .ok_or(TargetError::AddressNotFound(
                source.to_string_lossy().into_owned(),
                line,
            ))?;

        eprint!("BP at {}:{line} -> [{address:#x}]", source.display());

        Ok(self.connection.send(Request::SetBreakpoint(
            u32::try_from(address).map_err(|_| TargetError::AddressOutOfRange(address))?,
        ))?)
    }
}

impl From<MSIMError> for TargetError {
    fn from(error: MSIMError) -> Self {
        match error {
            MSIMError::RequestFailed(req_err) => Self::RequestFailed(req_err.into()),
            _ => Self::SessionLost,
        }
    }
}
