use crate::dap::context::Context;
use crate::msim::{MsimCommand, MsimResponse};
use dap::requests::{
    AttachRequestArguments, DisconnectArguments, InitializeArguments, SetBreakpointsArguments,
    SetExceptionBreakpointsArguments,
};
use dap::responses::{
    ResponseBody, SetBreakpointsResponse, SetExceptionBreakpointsResponse, ThreadsResponse,
};
use dap::types::{Breakpoint, Capabilities};
use std::path::Path;

pub trait Handles {
    fn initialize(&mut self, ctx: Context, args: &InitializeArguments) -> ResponseBody;
    fn attach(&mut self, ctx: Context, args: &AttachRequestArguments) -> ResponseBody;
    fn configuration_done(&mut self, ctx: Context) -> ResponseBody;
    fn set_breakpoints(&mut self, ctx: Context, args: &SetBreakpointsArguments) -> ResponseBody;
    fn set_exception_breakpoints(
        &mut self,
        ctx: Context,
        args: &SetExceptionBreakpointsArguments,
    ) -> ResponseBody;
    fn threads(&mut self, ctx: Context) -> ResponseBody;
    fn disconnect(&mut self, ctx: Context, args: &DisconnectArguments) -> ResponseBody;
}

pub struct Handler;

impl Handles for Handler {
    fn initialize(&mut self, _ctx: Context, args: &InitializeArguments) -> ResponseBody {
        if let Some(name) = &args.client_name {
            eprintln!("New client: {}, {}", name, args.adapter_id);
        }

        ResponseBody::Initialize(Capabilities {
            ..Default::default() // No extra capabilities advertised
        })
    }

    fn attach(&mut self, ctx: Context, _args: &AttachRequestArguments) -> ResponseBody {
        eprintln!("Attach request");
        ctx.server
            .send_event(dap::events::Event::Initialized)
            .expect("Server error");
        ResponseBody::Attach
    }
    fn configuration_done(&mut self, _ctx: Context) -> ResponseBody {
        ResponseBody::ConfigurationDone
    }

    fn set_breakpoints(&mut self, ctx: Context, args: &SetBreakpointsArguments) -> ResponseBody {
        let path = args.source.path.as_deref().unwrap_or("NO-PATH"); // TODO: some better default handling
        eprintln!("Path: {:?}", path);

        let bps = args.breakpoints.as_deref().unwrap_or(&[]);

        let mut set_bps = Vec::new();

        for bp in bps {
            let address = ctx.index.get_address(Path::new(&path), bp.line as u64);
            eprint!(
                "BP at {:?}:{}:{:?} -> [{:#x}]",
                path,
                bp.line,
                bp.column,
                address.unwrap_or(0)
            );
            if let Some(a) = address {
                let resp = ctx
                    .connection
                    .send_command(MsimCommand::SetBreakpoint(a as u32));

                match resp {
                    Ok(MsimResponse::Ok) => {
                        set_bps.push(Breakpoint {
                            id: None,
                            verified: true,
                            message: None,
                            source: None,
                            line: Some(bp.line),
                            column: bp.column,
                            end_line: None,
                            end_column: None,
                            instruction_reference: None,
                            offset: None,
                        });
                        eprintln!(" set")
                    }
                    _ => eprintln!(" NOT set"),
                }
            }
        }
        eprintln!();

        ResponseBody::SetBreakpoints(SetBreakpointsResponse {
            breakpoints: set_bps,
        })
    }

    fn set_exception_breakpoints(
        &mut self,
        _ctx: Context,
        _args: &SetExceptionBreakpointsArguments,
    ) -> ResponseBody {
        ResponseBody::SetExceptionBreakpoints(SetExceptionBreakpointsResponse {
            breakpoints: vec![].into(),
        })
    }

    fn threads(&mut self, _ctx: Context) -> ResponseBody {
        ResponseBody::Threads(ThreadsResponse { threads: vec![] })
    }
    fn disconnect(&mut self, _ctx: Context, _args: &DisconnectArguments) -> ResponseBody {
        ResponseBody::Disconnect
    }
}
