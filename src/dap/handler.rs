pub(crate) use crate::dap::context::Context;
use dap::requests::{
    AttachRequestArguments, DisconnectArguments, InitializeArguments, SetBreakpointsArguments,
    SetExceptionBreakpointsArguments,
};
use dap::responses::{
    ResponseBody, SetBreakpointsResponse, SetExceptionBreakpointsResponse, ThreadsResponse,
};
use dap::types::Capabilities;
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
            println!("New client: {}, {}", name, args.adapter_id);
        }

        ResponseBody::Initialize(Capabilities {
            ..Default::default() // No extra capabilities advertised
        })
    }

    fn attach(&mut self, ctx: Context, _args: &AttachRequestArguments) -> ResponseBody {
        println!("Attach request");
        ctx.server
            .send_event(dap::events::Event::Initialized)
            .expect("Server error");
        ResponseBody::Attach
    }
    fn configuration_done(&mut self, _ctx: Context) -> ResponseBody {
        ResponseBody::ConfigurationDone
    }

    fn set_breakpoints(&mut self, ctx: Context, args: &SetBreakpointsArguments) -> ResponseBody {
        let path = args.source.path.as_deref().unwrap_or("NO-PATH");
        println!("Path: {:?}", path);

        let bps = args.breakpoints.as_deref().unwrap_or(&[]);
        for bp in bps {
            let address = ctx.index.get_address(Path::new(&path), bp.line as u64);
            println!(
                "BP at {:?}:{}:{:?} -> [{:?}]",
                path, bp.line, bp.column, address
            );
        }
        println!();

        ResponseBody::SetBreakpoints(SetBreakpointsResponse {
            breakpoints: vec![],
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
