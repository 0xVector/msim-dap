mod context;
mod error;
mod handler;
mod server;
mod state;

use crate::dap::error::AdapterResult;
use crate::dap::handler::Handler;
use crate::dap::server::{serve, server_from_tcp};
use crate::dwarf::DwarfIndex;

pub fn run(index: DwarfIndex) -> AdapterResult<()> {
    let handler = Handler {};
    let mut server = server_from_tcp("127.0.0.1:15000")?;

    serve(handler, &mut server, &index)?;
    Ok(())
}
