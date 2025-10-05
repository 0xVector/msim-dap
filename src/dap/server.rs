use super::context::Context;
use super::handler::Handles;
use super::state::State;
use super::{DapError, Result};
use crate::dwarf::DwarfIndex;

use dap::prelude::ResponseBody;
use dap::requests::Command;
use dap::server::Server;
use std::io::{BufReader, BufWriter, Read, Write, stdin, stdout};
use std::net::{TcpListener, ToSocketAddrs};

pub type DapServer = Server<Box<dyn Read>, Box<dyn Write>>;

pub fn server_from_io<R, W>(r: R, w: W) -> Result<DapServer>
where
    R: Read + 'static,
    W: Write + 'static,
{
    Ok(Server::new(
        BufReader::new(Box::new(r)),
        BufWriter::new(Box::new(w)),
    ))
}

pub fn server_from_stdio() -> Result<DapServer> {
    server_from_io(stdin(), stdout())
}

pub fn server_from_tcp(address: impl ToSocketAddrs) -> Result<DapServer> {
    let listener = TcpListener::bind(address)?;
    let (stream, _addr) = listener.accept()?;
    server_from_io(stream.try_clone()?, stream)
}

pub fn serve(mut handler: impl Handles, server: &mut DapServer, index: &DwarfIndex) -> Result<()> {
    let mut state = State::New;

    while let Some(req) = server.poll_request()? {
        let ctx = Context::new(&mut state, server, index);

        let resp_body: ResponseBody = match &req.command {
            Command::Initialize(args) => handler.initialize(ctx, args),
            Command::Attach(args) => handler.attach(ctx, args),
            Command::ConfigurationDone => handler.configuration_done(ctx),
            Command::SetBreakpoints(args) => handler.set_breakpoints(ctx, args),
            Command::SetExceptionBreakpoints(args) => handler.set_exception_breakpoints(ctx, args),
            Command::Threads => handler.threads(ctx),
            Command::Disconnect(args) => handler.disconnect(ctx, args),
            _ => return Err(DapError::UnhandledCommandError),
        };

        let resp = req.success(resp_body);
        server.respond(resp)?;
    }

    Ok(())
}
