use crate::dwarf::DwarfIndex;
use crate::dap::error::AdapterError::UnhandledCommandError;
use crate::dap::error::AdapterResult;
use crate::dap::handler::{Handler, Handles};
use dap::prelude::ResponseBody;
use dap::requests::Command;
use dap::server::Server;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};

pub type DapServer = Server<TcpStream, TcpStream>;

pub fn create(address: &str) -> AdapterResult<DapServer> {
    let listener = TcpListener::bind(address)?;
    let (stream, _addr) = listener.accept()?;
    let reader = BufReader::new(stream.try_clone()?);
    let writer = BufWriter::new(stream);

    Ok(Server::new(reader, writer))
}

pub fn serve<H: Handles>(server: &mut DapServer, index: DwarfIndex) -> AdapterResult<()> {
    let mut handler = Handler::new(index);

    while let Some(req) = server.poll_request()? {
        let resp_body: ResponseBody = match &req.command {
            Command::Initialize(args) => handler.initialize(server, args),
            Command::Attach(args) => handler.attach(server, args),
            Command::ConfigurationDone => handler.configuration_done(server),
            Command::SetBreakpoints(args) => handler.set_breakpoints(server, args),
            Command::SetExceptionBreakpoints(args) => {
                handler.set_exception_breakpoints(server, args)
            }
            Command::Threads => handler.threads(server),
            Command::Disconnect(args) => handler.disconnect(server, args),
            _ => return Err(UnhandledCommandError),
        };

        let resp = req.success(resp_body);
        server.respond(resp)?;
    }

    Ok(())
}
