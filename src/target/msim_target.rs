use super::{DebugTarget, Result, TargetError};
use crate::dwarf::DwarfIndex;
use crate::msim::{Connection, MsimError, Request};
use crate::{Address, LineNo};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct MsimTarget<S: Connection> {
    connection: S,
    index: DwarfIndex,
    bp_store: BpStore,
}

#[derive(Default)]
struct BpStore {
    bps_per_file: HashMap<PathBuf, Vec<(LineNo, Address)>>,
    bps_addresses: HashSet<Address>,
}

impl<S: Connection> MsimTarget<S> {
    pub fn new(session: S, index: DwarfIndex) -> Self {
        Self {
            connection: session,
            index,
            bp_store: BpStore::default(),
        }
    }
}

impl<S: Connection> DebugTarget for MsimTarget<S> {
    fn resume(&mut self) -> Result<()> {
        Ok(self.connection.send(Request::Resume)?.get_result()?)
    }

    fn pause(&mut self) -> Result<()> {
        Ok(self.connection.send(Request::Pause)?.get_result()?)
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

    // TODO: if a BP is changed while step is in progress, replace fn will overwrite the step BP. Solving needs a flag but prolly not realistic.
    fn set_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        // BP already exists, just return the address
        if self.bp_store.bps_addresses.contains(&address) {
            return Ok(address);
        }

        self.connection
            .send(Request::SetCodeBreakpoint(address))?
            .get_result()?;
        self.bp_store.bps_addresses.insert(address);

        Ok(address)
    }

    fn remove_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        if self.bp_store.bps_addresses.remove(&address) {
            self.connection
                .send(Request::RemoveCodeBreakpoint(address))?
                .get_result()?;
        }

        Ok(address)
    }

    fn replace_code_bps(&mut self, source: &Path, lines: &[LineNo]) -> Vec<Result<()>> {
        let old_by_file: HashMap<LineNo, Address> = self
            .bp_store
            .bps_per_file
            .remove(source)
            .unwrap_or_default()
            .into_iter()
            .collect();

        // Remove old BPs that are not in the new set
        for (&line, &_addr) in &old_by_file {
            if !lines.contains(&line) {
                self.remove_code_bp(source, line).ok(); // TODO: handle error somehow
            }
        }

        let mut new_by_file = Vec::with_capacity(lines.len());
        let mut results = Vec::with_capacity(lines.len());

        for &line in lines {
            let result: Result<()> = (|| {
                // Reuse old BP if it exists
                if let Some(&addr) = old_by_file.get(&line) {
                    new_by_file.push((line, addr));
                    return Ok(());
                }

                let address = self.set_code_bp(source, line)?;
                new_by_file.push((line, address));
                Ok(())
            })();

            results.push(result);
            if matches!(results.last(), Some(Err(TargetError::SessionLost))) {
                break;
            }
        }

        if !new_by_file.is_empty() {
            self.bp_store
                .bps_per_file
                .insert(source.to_path_buf(), new_by_file);
        }
        results
    }

    fn resolve_code_bp(&self, address: Address) -> Option<(&Path, LineNo)> {
        if self.bp_store.bps_addresses.contains(&address) {
            self.index.resolve_address(address)
        } else {
            None
        }
    }

    fn resolve_address(&self, address: Address) -> Option<(&Path, LineNo)> {
        self.index.resolve_address(address)
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
