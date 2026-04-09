use super::core::{HandlerAction, PostAction};
use super::{Debugger, DebuggerError, Result};
use crate::debugger::core::PostAction::Disconnect;
use crate::target::{DebugTarget, TargetError};
use dap::prelude::ResponseBody;
use dap::requests::{
    AttachRequestArguments, DisconnectArguments, InitializeArguments, LaunchRequestArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments,
};
use dap::responses::{SetBreakpointsResponse, SetExceptionBreakpointsResponse, ThreadsResponse};
use dap::types::{Breakpoint, Capabilities};
use std::path::Path;

type HandlerResult = Result<HandlerAction>;

// DAP request handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    pub(super) fn initialize(&mut self, args: &InitializeArguments) -> HandlerResult {
        if let Some(name) = &args.client_name {
            eprintln!("New client: {name}, {}", args.adapter_id);
        }

        Ok(HandlerAction {
            body: ResponseBody::Initialize(Capabilities {
                supports_configuration_done_request: Some(true),
                ..Default::default() // No extra capabilities advertised
            }),
            post_action: Some(PostAction::SendEvent(dap::events::Event::Initialized)),
        })
    }

    pub(super) fn attach(&mut self, _args: &AttachRequestArguments) -> HandlerResult {
        eprintln!("Attach request");
        Ok(HandlerAction {
            body: ResponseBody::Attach,
            post_action: None,
        })
    }

    pub(super) fn launch(&mut self, _args: &LaunchRequestArguments) -> HandlerResult {
        eprintln!("Launch request");
        Ok(HandlerAction {
            body: ResponseBody::Launch,
            post_action: None,
        })
    }

    pub(super) fn configuration_done(&mut self) -> HandlerResult {
        // TODO: maybe its not universal/shouldn't be to resume on startup
        self.target.resume()?;

        Ok(HandlerAction {
            body: ResponseBody::ConfigurationDone,
            post_action: None,
        })
    }

    pub(super) fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> HandlerResult {
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

        Ok(HandlerAction {
            body: ResponseBody::SetBreakpoints(SetBreakpointsResponse {
                breakpoints: set_bps,
            }),
            post_action: None,
        })
    }

    pub(super) fn set_exception_breakpoints(
        &mut self,
        _args: &SetExceptionBreakpointsArguments,
    ) -> HandlerResult {
        Ok(HandlerAction {
            body: ResponseBody::SetExceptionBreakpoints(SetExceptionBreakpointsResponse {
                breakpoints: vec![].into(),
            }),
            post_action: None,
        })
    }

    pub(super) const fn threads(&mut self) -> HandlerResult {
        Ok(HandlerAction {
            body: ResponseBody::Threads(ThreadsResponse { threads: vec![] }),
            post_action: None,
        })
    }

    pub(super) fn disconnect(&mut self, _args: &DisconnectArguments) -> HandlerResult {
        self.target.stop().ok(); // Best effort to stop target, ignore errors since we're disconnecting anyway

        Ok(HandlerAction {
            body: ResponseBody::Disconnect,
            post_action: Some(Disconnect), // Signal to stop the event loop
        })
    }
}
