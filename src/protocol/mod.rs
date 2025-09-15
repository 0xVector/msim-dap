mod protocol;
mod error;
mod handler;
mod state;

use crate::protocol::error::AdapterResult;
use crate::protocol::handler::Handler;
use crate::protocol::protocol::{create, serve};

pub fn run() -> AdapterResult<()> {
    let mut server = create("127.0.0.1:15000")?;
    serve::<Handler>(&mut server)?;
    
    Ok(())
}

// #[derive(Error, Debug)]
// enum MyAdapterError {
//     #[error("Unhandled command")]
//     UnhandledCommandError,
//
//     #[error("Missing command")]
//     MissingCommandError,
// }

// type DynResult<T> = Result<T, Box<dyn std::error::Error>>;
//
// pub fn run() -> DynResult<()> {
//     let listener = TcpListener::bind("127.0.0.1:15000")?;
//     let (stream, _addr) = listener.accept()?;
//     let reader = BufReader::new(stream.try_clone()?);
//     let writer = BufWriter::new(stream);
//
//     // let output = BufWriter::new(std::io::stdout());
//     // let f = File::open("testinput.txt")?;
//     // let input = BufReader::new(f);
//     let mut server = Server::new(reader, writer);
//     println!("Waiting for connection...");
//
//     let req = match server.poll_request()? {
//         Some(req) => req,
//         None => return Err(Box::new(MyAdapterError::MissingCommandError)),
//     };
//
//     if let Command::Initialize(args) = &req.command {
//         if let Some(name) = &args.client_name {
//             println!("New client: {}, {}", name, args.adapter_id);
//         }
//
//         let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
//             ..Default::default() // No extra capabilities advertised
//         }));
//
//         // When you call respond, send_event etc. the message will be wrapped
//         // in a base message with a appropriate seq number, so you don't have to keep track of that yourself
//         server.respond(rsp)?;
//     } else {
//         return Err(Box::new(MyAdapterError::UnhandledCommandError));
//     }
//
//     // Attach
//     let req = match server.poll_request()? {
//         Some(req) => req,
//         None => return Err(Box::new(MyAdapterError::MissingCommandError)),
//     };
//
//     if let Command::Attach(_args) = &req.command {
//         println!("Attach request");
//         let rsp = req.success(ResponseBody::Attach);
//         server.respond(rsp)?;
//     } else {
//         return Err(Box::new(MyAdapterError::UnhandledCommandError));
//     }
//
//     // Send initialized
//     server.send_event(Event::Initialized)?;
//
//     // Config
//     conf(&mut server)
// }
//
// fn conf(server: &mut Server<TcpStream, TcpStream>) -> DynResult<()> {
//     loop {
//         let r = server.poll_request();
//         let req = match r? {
//             Some(req) => req,
//             None => return Ok(())
//             // None => return Err(Box::new(MyAdapterError::MissingCommandError)),
//         };
//
//         if let Command::ConfigurationDone = &req.command {
//             println!("Conf done");
//             let rsp = req.success(ResponseBody::Attach);
//             server.respond(rsp)?;
//         } else if let Command::SetBreakpoints(args) = &req.command {
//             println!("Set breakpoints");
//             let rsp = req.success(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
//                 breakpoints: vec![],
//             }));
//             server.respond(rsp)?;
//         } else if let Command::SetExceptionBreakpoints(args) = &req.command {
//             println!("Set exception breakpoints");
//             let rsp = req.success(ResponseBody::SetExceptionBreakpoints(
//                 SetExceptionBreakpointsResponse {
//                     breakpoints: vec![].into(),
//                 },
//             ));
//             server.respond(rsp)?;
//         } else if let Command::Threads = &req.command {
//             println!("Threads");
//             let rsp = req.success(ResponseBody::Threads(ThreadsResponse { threads: vec![] }));
//             server.respond(rsp)?;
//         } else if let Command::Disconnect(args) = &req.command {
//             println!("Client disconnect");
//             let rsp = req.success(ResponseBody::Disconnect);
//             server.respond(rsp)?;
//         } else {
//             return Err(Box::new(MyAdapterError::UnhandledCommandError));
//         }
//     }
// }
