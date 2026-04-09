use super::{DebugTarget, Result, TargetError};
use crate::dwarf::DwarfIndex;
use crate::msim::{Connection, MsimError, Request};
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
            | Err(MsimError::ListenerDied | MsimError::ClosedError | MsimError::IOError(_)) => {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn set_breakpoint(&mut self, source: &Path, line: u64) -> Result<()> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        eprintln!("BP at {}:{line} -> [{address:#x}]", source.display());
        Ok(self
            .connection
            .send(Request::SetCodeBreakpoint(address))?
            .get_result()?)
    }
}

impl From<MsimError> for TargetError {
    fn from(error: MsimError) -> Self {
        // Default to SessionLost (fatal), only RequestFailed is recoverable
        match error {
            MsimError::RequestFailed => Self::RequestFailed,
            _ => Self::SessionLost,
        }
    }
}
