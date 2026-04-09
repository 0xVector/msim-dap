use super::{Debugger, DebuggerError, Result};
use crate::target::{DebugTarget, TargetError};
use dap::base_message::Sendable::Event;
use dap::prelude::ResponseBody;
use dap::requests::{
    AttachRequestArguments, DisconnectArguments, InitializeArguments, LaunchRequestArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments,
};
use dap::responses::{SetBreakpointsResponse, SetExceptionBreakpointsResponse, ThreadsResponse};
use dap::types::{Breakpoint, Capabilities};
use std::path::Path;

// DAP request handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    pub(super) fn initialize(&mut self, args: &InitializeArguments) -> Result<ResponseBody> {
        if let Some(name) = &args.client_name {
            eprintln!("New client: {name}, {}", args.adapter_id);
        }

        Ok(ResponseBody::Initialize(Capabilities {
            supports_configuration_done_request: Some(true),
            ..Default::default() // No extra capabilities advertised
        }))
    }

    pub(super) fn attach(&mut self, _args: &AttachRequestArguments) -> Result<ResponseBody> {
        eprintln!("Attach request");
        // TODO: move to initialize to be spec-consistent
        self.dap_session
            .send(Event(dap::events::Event::Initialized))?;

        Ok(ResponseBody::Attach)
    }

    pub(super) fn launch(&mut self, _args: &LaunchRequestArguments) -> Result<ResponseBody> {
        eprintln!("Launch request");
        // TODO: move to initialize to be spec-consistent
        self.dap_session
            .send(Event(dap::events::Event::Initialized))?;
        Ok(ResponseBody::Launch)
    }

    pub(super) fn configuration_done(&mut self) -> Result<ResponseBody> {
        // TODO: maybe its not universal/shouldn't be to resume on startup

        self.target.resume()?;
        Ok(ResponseBody::ConfigurationDone)
    }

    pub(super) fn set_breakpoints(
        &mut self,
        args: &SetBreakpointsArguments,
    ) -> Result<ResponseBody> {
        let path = args
            .source
            .path
            .as_deref()
            .ok_or(DebuggerError::RequestFailed(
                "Source path is required for breakpoints".into(),
            ))?;
        eprintln!("Path: {path}");

        let bps = args.breakpoints.as_deref().unwrap_or(&[]);

        let mut set_bps = Vec::new();

        for bp in bps {
            let mut bp_info = Breakpoint {
                id: None,
                verified: false,
                message: None,
                source: None,
                line: Some(bp.line),
                column: bp.column,
                end_line: None,
                end_column: None,
                instruction_reference: None,
                offset: None,
            };

            let res = self
                .target
                .set_breakpoint(Path::new(&path), bp.line.cast_unsigned());
            match res {
                Ok(()) => {
                    bp_info.verified = true;
                }

                Err(e) => match e {
                    // Fatal
                    TargetError::SessionLost => {
                        return Err(DebuggerError::SessionLost);
                    }

                    // Recoverable
                    TargetError::RequestFailed => {
                        let msg = format!("Setting BP {path}:{} failed!", bp.line);
                        eprintln!("{msg}");
                        bp_info.message = Some(msg);
                    }

                    TargetError::AddressNotFound(path, line) => {
                        let msg = format!("Address not found for {path}:{line}");
                        eprintln!("{msg}");
                        bp_info.message = Some(msg);
                    }
                    TargetError::AddressOutOfRange(a) => {
                        let msg = format!("Address out of range for {path}:{} ({a})", bp.line);
                        eprintln!("{msg}");
                        bp_info.message = Some(msg);
                    }
                },
            }

            set_bps.push(bp_info);
        }

        Ok(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
            breakpoints: set_bps,
        }))
    }

    pub(super) fn set_exception_breakpoints(
        &mut self,
        _args: &SetExceptionBreakpointsArguments,
    ) -> Result<ResponseBody> {
        Ok(ResponseBody::SetExceptionBreakpoints(
            SetExceptionBreakpointsResponse {
                breakpoints: vec![].into(),
            },
        ))
    }

    pub(super) const fn threads(&mut self) -> Result<ResponseBody> {
        Ok(ResponseBody::Threads(ThreadsResponse { threads: vec![] }))
    }

    pub(super) fn disconnect(&mut self, _args: &DisconnectArguments) -> Result<ResponseBody> {
        self.target.stop().ok(); // Best effort to stop target, ignore errors since we're disconnecting anyway
        Err(DebuggerError::DapDisconnected)
    }
}
