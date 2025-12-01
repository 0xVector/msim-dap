//! MSIM DAP adapter
use crate::dap::server_from_stdio;
use crate::dwarf::parse_dwarf;
use crate::msim::TcpMsimConnection;
use dap::{Handler, serve, server_from_tcp};
use std::path::Path;
use thiserror::Error;

mod dap;
mod dwarf;
mod msim;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("DWARF error: {0}")]
    Dwarf(#[from] dwarf::DwarfError),

    #[error("DAP protocol error: {0}")]
    DAP(#[from] dap::DapError),

    #[error("MSIM error: {0}")]
    MSIM(#[from] msim::MSIMError),
}

/// MSIM-DAP library error type
pub type Result<T> = std::result::Result<T, Error>;

/// Port number
type Port = u16;

/// DAP layer mode
#[derive(Debug)]
pub enum Mode {
    /// stdio DAP mode
    Stdio,

    /// TCP DAP mode, with port number
    TCP(Port),
}

/// Adapter config
#[derive(Debug)]
pub struct Config<'a> {
    /// Mode to use for the DAP layer
    pub mode: Mode,

    /// MSIM TCP connection port to use
    pub msim_port: Port,

    /// Path to the kernel.raw file
    pub kernel_path: &'a Path,
}

/// Run with config
pub fn run(config: &Config) -> Result<()> {
    eprintln!("Parsing dwarf...");
    let index = parse_dwarf(config.kernel_path)?;

    eprintln!("Starting up DAP server...");
    let mut server = match config.mode {
        Mode::Stdio => server_from_stdio(),
        Mode::TCP(port) => {
            let address = format!("127.0.0.1:{}", port);
            eprintln!("Waiting for DAP connection on {}", address);
            server_from_tcp(address)
        }
    }?;

    eprintln!("Connecting to MSIM...");
    let mut msim_connection = TcpMsimConnection::new(config.msim_port)?;

    let mut handler = Handler {};

    eprintln!("Ready!");
    serve(&mut handler, &mut server, &mut msim_connection, &index)?;
    Ok(())
}
