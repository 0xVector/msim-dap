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
    AddressNotFound(String, u64),

    #[error("Address {0} is out of range")]
    AddressOutOfRange(u64),
}

pub trait DebugTarget {
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn set_breakpoint(&mut self, source: &Path, line: u64) -> Result<()>;
}
