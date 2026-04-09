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
        Ok(self.connection.send(Request::Resume)?.get_result()?)
    }

    fn stop(&mut self) -> Result<()> {
        match self.connection.send(Request::Stop) {
            // We treat these errors as success
            Ok(_)
            | Err(MSIMError::ListenerDied)
            | Err(MSIMError::ClosedError)
            | Err(MSIMError::IOError(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn set_breakpoint(&mut self, source: &Path, line: u64) -> Result<()> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        eprint!("BP at {}:{line} -> [{address:#x}]", source.display());
        Ok(self
            .connection
            .send(Request::SetCodeBreakpoint(address))?
            .get_result()?)
    }
}

impl From<MSIMError> for TargetError {
    fn from(error: MSIMError) -> Self {
        // Default to SessionLost (fatal), only RequestFailed is recoverable
        match error {
            MSIMError::RequestFailed => Self::RequestFailed,
            _ => Self::SessionLost,
        }
    }
}
