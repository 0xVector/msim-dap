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

    fn replace_code_bps(&mut self, source: &Path, lines: &[LineNo]) -> Vec<Result<()>> {
        let old_by_file: HashMap<LineNo, Address> = self
            .bp_store
            .bps_per_file
            .remove(source)
            .unwrap_or_default()
            .into_iter()
            .collect();

        for (&line, &addr) in &old_by_file {
            if !lines.contains(&line) {
                self.bp_store.bps_addresses.remove(&addr);
                let _ = self.connection.send(Request::RemoveCodeBreakpoint(addr));
            }
        }

        let mut new_by_file = Vec::with_capacity(lines.len());
        let mut results = Vec::with_capacity(lines.len());

        for &line in lines {
            let result: Result<()> = (|| {
                if let Some(&addr) = old_by_file.get(&line) {
                    new_by_file.push((line, addr));
                    return Ok(());
                }

                let addr = self.index.get_address(source, line).ok_or_else(|| {
                    TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
                })?;

                self.connection
                    .send(Request::SetCodeBreakpoint(addr))?
                    .get_result()?;
                self.bp_store.bps_addresses.insert(addr);
                new_by_file.push((line, addr));
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
