use super::core::{HandlerAction, PostAction, PostAction::Disconnect};
use super::{Debugger, DebuggerError, Result};
use crate::debugger::DebuggerError::RequestFailed;
use crate::target::{DebugTarget, TargetError};
use crate::LineNo;
use dap::prelude::ResponseBody;
use dap::requests::{
    AttachRequestArguments, ContinueArguments, DisconnectArguments, InitializeArguments,
    LaunchRequestArguments, NextArguments, PauseArguments, ScopesArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments, StackTraceArguments,
    StepInArguments,
};
use dap::responses::{
    SetBreakpointsResponse, SetExceptionBreakpointsResponse, StackTraceResponse, ThreadsResponse,
};
use dap::types::{Breakpoint, Capabilities, Source, StackFrame, Thread};
use std::iter::zip;
use std::path::Path;

type HandlerResult = Result<HandlerAction>;

/// How many lines to search for a valid step over target when stepping over
const STEP_OVER_LINE_SEARCH_LIMIT: LineNo = 20;

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

        let bps = args.breakpoints.as_deref().unwrap_or(&[]);
        let mut set_bps = Vec::new();
        eprintln!("Setting {} BPs for file: {path}", bps.len());

        let results = self.target.replace_code_bps(
            Path::new(path),
            &bps.iter().map(|bp| bp.line).collect::<Vec<_>>(),
        );

        for (bp, result) in zip(bps, results) {
            let mut bp_info = Breakpoint {
                id: result
                    .is_ok()
                    .then(|| i64::from(self.bp_registry.get_id(Path::new(path), bp.line))),
                verified: result.is_ok(),
                message: None,
                source: None,
                line: Some(bp.line),
                column: bp.column,
                end_line: None,
                end_column: None,
                instruction_reference: None,
                offset: None,
            };

            match result {
                Ok(()) => {
                    eprintln!("Set BP at {path}:{} (ID {:?})", bp.line, bp_info.id);
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

                    TargetError::AddressNotFound(p, line) => {
                        let msg = format!("Address not found for {p}:{line}");
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

    pub(super) const fn set_exception_breakpoints(
        &mut self,
        _args: &SetExceptionBreakpointsArguments,
    ) -> HandlerResult {
        Ok(HandlerAction {
            body: ResponseBody::SetExceptionBreakpoints(SetExceptionBreakpointsResponse {
                breakpoints: Some(vec![]),
            }),
            post_action: None,
        })
    }

    pub(super) fn threads(&mut self) -> HandlerResult {
        let cpu_count = self.target.cpu_count().unwrap_or(1).cast_signed();

        Ok(HandlerAction {
            body: ResponseBody::Threads(ThreadsResponse {
                threads: (0..cpu_count)
                    .map(|i| Thread {
                        id: i + 1,
                        name: format!("CPU {i}"),
                    })
                    .collect(),
            }),
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

    pub(super) fn stack_trace(&mut self, args: &StackTraceArguments) -> HandlerResult {
        let (path, line) = self
            .last_stopped_at
            .and_then(|addr| self.target.resolve_address(addr))
            .map_or((None, 0), |(path, line)| {
                (Some(path.to_string_lossy().into_owned()), line)
            });
        eprintln!(
            "Stack trace requested, last stopped at: {} ({}:{})",
            self.last_stopped_at
                .map_or_else(|| "<unknown>".into(), |a| format!("{a:#x}")),
            path.as_deref().unwrap_or("<unknown>"),
            line
        );

        Ok(HandlerAction {
            body: ResponseBody::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: args.thread_id,
                    name: "<unknown>".into(),
                    source: Some(Source {
                        path,
                        ..Default::default()
                    }),
                    line,
                    column: 0,
                    ..Default::default()
                }],
                total_frames: Some(1),
            }),
            post_action: None,
        })
    }

    pub(super) const fn scopes(&mut self, _args: &ScopesArguments) -> HandlerResult {
        Ok(HandlerAction {
            body: ResponseBody::Scopes(dap::responses::ScopesResponse { scopes: vec![] }),
            post_action: None,
        })
    }

    pub(super) fn resume(&mut self, _args: &ContinueArguments) -> HandlerResult {
        self.target.resume()?;
        Ok(HandlerAction {
            body: ResponseBody::Continue(dap::responses::ContinueResponse::default()),
            post_action: None,
        })
    }

    pub(super) fn pause(&mut self, _args: &PauseArguments) -> HandlerResult {
        self.target.pause()?;
        Ok(HandlerAction {
            body: ResponseBody::Pause,
            post_action: None,
        })
    }

    pub(super) fn next(&mut self, _args: &NextArguments) -> HandlerResult {
        let address = self.last_stopped_at.ok_or(RequestFailed(
            "Cannot step because the target is not paused".into(),
        ))?;
        let (path, line) = self
            .target
            .resolve_address(address)
            .ok_or(RequestFailed(
                "Cannot step because the target address cannot be resolved".into(),
            ))
            .map(|(p, l)| (p.to_owned(), l))?;

        // Set temporary BP at target location to know when the step is complete
        for offset in 1..STEP_OVER_LINE_SEARCH_LIMIT {
            let next_line = line + offset;
            let next_address = match self.target.set_code_bp(path.as_path(), next_line) {
                Ok(a) => a,
                Err(TargetError::SessionLost) => return Err(DebuggerError::SessionLost),
                Err(_) => continue, // Try next line if setting BP failed (e.g. no
            };

            self.step_bp = Some(next_address);

            eprintln!(
                "Step Over: {}:{line} (address {:#x}) -> :{} (address {:#x})",
                path.display(),
                address,
                next_line,
                next_address
            );
            self.target.resume()?;

            return Ok(HandlerAction {
                body: ResponseBody::Next,
                post_action: None,
            });
        }

        Err(RequestFailed(
            "Cannot step because no valid next line was found".into(),
        ))
    }

    pub(super) fn step_in(&mut self, _args: &StepInArguments) -> HandlerResult {
        self.target.step_by(1)?;

        Ok(HandlerAction {
            body: ResponseBody::StepIn,
            post_action: None,
        })
    }
}
