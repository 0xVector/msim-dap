use crate::protocol::error::AdapterError::UnhandledCommandError;
use crate::protocol::error::AdapterResult;
use crate::protocol::handler::Handles;
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

pub fn serve<H: Handles>(server: &mut DapServer) -> AdapterResult<()> {
    while let Some(req) = server.poll_request()? {
        let resp_body: ResponseBody = match &req.command {
            Command::Initialize(args) => H::initialize(server, args),
            _ => return Err(UnhandledCommandError),
        };

        let resp = req.success(resp_body);
        server.respond(resp)?;
    }

    Ok(())
}
