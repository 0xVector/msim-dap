//! MSIM DAP adapter
use crate::adapter::Session;
use crate::debugger::Debugger;
use crate::dwarf::parse_dwarf;
use crate::msim::TcpConnection;
use crate::target::MsimTarget;
use std::path::Path;
use thiserror::Error;

mod adapter;
mod debugger;
mod dwarf;
mod msim;
mod target;

/// MSIM-DAP library error type
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("DWARF error: {0}")]
    Dwarf(#[from] dwarf::DwarfError),

    #[error("DAP protocol error: {0}")]
    Adapter(#[from] adapter::AdapterError),

    #[error("MSIM error: {0}")]
    Msim(#[from] msim::MsimError),

    #[error("Debugger error: {0}")]
    Debugger(#[from] debugger::DebuggerError),
}

/// MSIM-DAP library result
pub type Result<T> = std::result::Result<T, Error>;

/// Address type (64-bit)
pub type Address = u64;

/// Port number
type Port = u16;

/// DAP layer mode
#[derive(Debug)]
pub enum Mode {
    /// stdio DAP mode
    Stdio,

    /// TCP DAP mode, with port number
    Tcp(Port),
}

/// Adapter config
#[derive(Debug)]
pub struct Config<'a> {
    /// Mode to use for the DAP layer
    pub mode: Mode,

    /// MSIM TCP connection port to use
    pub msim_port: Port,

    /// Path to the `kernel.raw` file
    pub kernel_path: &'a Path,
}

/// General event type for the debugger, which can be:
/// - either a DAP request from the adapter,
/// - or an MSIM event from the target session
#[allow(clippy::large_enum_variant)] // Unnecessary to box here
pub enum DebugEvent {
    DapRequest(dap::requests::Request),
    MsimEvent(msim::Event),
}
type AnyError = Box<dyn std::error::Error + Send + Sync>;
type DebugEventResult = std::result::Result<DebugEvent, AnyError>;
type DebugEventSender = std::sync::mpsc::Sender<DebugEventResult>;
type DebugEventReceiver = std::sync::mpsc::Receiver<DebugEventResult>;

/// Run with config
/// # Errors
/// Fatal errors from the adapter, debugger, MSIM connection, or DWARF parsing
pub fn run(config: &Config) -> Result<()> {
    eprintln!("Parsing dwarf...");
    let index = parse_dwarf(config.kernel_path)?;

    let (tx, rx) = std::sync::mpsc::channel();

    eprintln!("Starting up DAP session...");
    let dap_session = match config.mode {
        Mode::Stdio => Session::session_from_stdio(tx.clone()),
        Mode::Tcp(port) => {
            let address = format!("127.0.0.1:{port}");
            eprintln!("Waiting for DAP connection on {address}");
            Session::session_from_tcp(address, tx.clone())?
        }
    };

    eprintln!("Connecting to MSIM...");
    let connection = TcpConnection::connect(config.msim_port, tx)?;
    let target = MsimTarget::new(connection, index);

    let mut debugger = Debugger::new(rx, dap_session, target);

    eprintln!("Ready!");
    debugger.run().map_err(Error::Debugger)
}
