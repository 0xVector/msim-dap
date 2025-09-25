use crate::parser::DwarfIndex;
use crate::protocol::protocol::DapServer;
use crate::protocol::state::State;
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
    fn state(&self) -> State;
    fn index(&self) -> &DwarfIndex;

    fn initialize(&mut self, server: &mut DapServer, args: &InitializeArguments) -> ResponseBody;
    fn attach(&mut self, server: &mut DapServer, args: &AttachRequestArguments) -> ResponseBody;
    fn configuration_done(&mut self, server: &mut DapServer) -> ResponseBody;
    fn set_breakpoints(
        &mut self,
        server: &mut DapServer,
        args: &SetBreakpointsArguments,
    ) -> ResponseBody;
    fn set_exception_breakpoints(
        &mut self,
        server: &mut DapServer,
        args: &SetExceptionBreakpointsArguments,
    ) -> ResponseBody;
    fn threads(&mut self, server: &mut DapServer) -> ResponseBody;
    fn disconnect(&mut self, server: &mut DapServer, args: &DisconnectArguments) -> ResponseBody;
}

pub struct Handler {
    state: State,
    index: DwarfIndex,
}

impl Handler {
    pub fn new(index: DwarfIndex) -> Self {
        Handler {
            state: State::New,
            index,
        }
    }
}

impl Handles for Handler {
    fn state(&self) -> State {
        self.state
    }

    fn index(&self) -> &DwarfIndex {
        &self.index
    }

    fn initialize(&mut self, _server: &mut DapServer, args: &InitializeArguments) -> ResponseBody {
        if let Some(name) = &args.client_name {
            println!("New client: {}, {}", name, args.adapter_id);
        }

        ResponseBody::Initialize(Capabilities {
            ..Default::default() // No extra capabilities advertised
        })
    }

    fn attach(&mut self, server: &mut DapServer, _args: &AttachRequestArguments) -> ResponseBody {
        println!("Attach request");
        server
            .send_event(dap::events::Event::Initialized)
            .expect("Server error");
        ResponseBody::Attach
    }
    fn configuration_done(&mut self, server: &mut DapServer) -> ResponseBody {
        ResponseBody::ConfigurationDone
    }

    fn set_breakpoints(
        &mut self,
        server: &mut DapServer,
        args: &SetBreakpointsArguments,
    ) -> ResponseBody {
        let path = args.source.path.as_deref().unwrap_or("NO-PATH");
        println!("Path: {:?}", path);

        let bps = args.breakpoints.as_deref().unwrap_or(&[]);
        for bp in bps {
            let address = self.index.get_address(Path::new(&path), bp.line as u64);
            println!(
                "BP at {:?}:{}:{:?} -> [{:?}]",
                path, bp.line, bp.column, address
            );
        }

        ResponseBody::SetBreakpoints(SetBreakpointsResponse {
            breakpoints: vec![],
        })
    }

    fn set_exception_breakpoints(
        &mut self,
        server: &mut DapServer,
        args: &SetExceptionBreakpointsArguments,
    ) -> ResponseBody {
        ResponseBody::SetExceptionBreakpoints(SetExceptionBreakpointsResponse {
            breakpoints: vec![].into(),
        })
    }

    fn threads(&mut self, server: &mut DapServer) -> ResponseBody {
        ResponseBody::Threads(ThreadsResponse { threads: vec![] })
    }
    fn disconnect(&mut self, server: &mut DapServer, args: &DisconnectArguments) -> ResponseBody {
        ResponseBody::Disconnect
    }
}
