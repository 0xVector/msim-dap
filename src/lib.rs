//! MSIM DAP adapter
use crate::dap::server_from_stdio;
use crate::dwarf::parse_dwarf;
use dap::{Handler, server_from_tcp, serve};
use std::path::Path;
use thiserror::Error;
use crate::msim::Commander;

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
    println!("Parsing dwarf...");
    let index = parse_dwarf(config.kernel_path)?;

    println!("Starting up DAP server...");
    let mut server = match config.mode {
        Mode::Stdio => server_from_stdio(),
        Mode::TCP(_) => server_from_tcp("127.0.0.1:15000"),
    }?;

    println!("Connecting to MSIM...");
    let mut commander = Commander::new(config.msim_port)?;

    let mut handler = Handler {};
    
    println!("Ready!");
    serve(&mut handler, &mut server, &mut commander, &index)?;
    Ok(())
}
