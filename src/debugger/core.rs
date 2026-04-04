use super::{DebuggerError, Result};
use crate::target::{DebugTarget, TargetError};
use crate::{Address, DebugEvent, DebugEventReceiver};
use crate::{adapter, msim};
use dap::base_message::Sendable::{Event, Response};
use dap::prelude::{Command, ResponseBody};
use dap::requests::{
    AttachRequestArguments, DisconnectArguments, InitializeArguments, LaunchRequestArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments,
};
use dap::responses::{SetBreakpointsResponse, SetExceptionBreakpointsResponse, ThreadsResponse};
use dap::types::{Breakpoint, Capabilities};
use std::path::Path;

pub struct Debugger<T: DebugTarget> {
    receiver: DebugEventReceiver,
    dap_session: adapter::Session,
    target: T,
}

impl<T: DebugTarget> Debugger<T> {
    pub fn new(
        receiver: DebugEventReceiver,
        dap_session: adapter::Session,
        msim_session: T,
    ) -> Self {
        Self {
            receiver,
            dap_session,
            target: msim_session,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            match self.receiver.recv() {
                Ok(Ok(debug_event)) => self.handle_event(debug_event)?,

                // Received fatal error from listener, log it and exit
                Ok(Err(fatal_err)) => {
                    eprintln!("Received fatal error from listener: {fatal_err}");
                    return Err(DebuggerError::ReceivedFatalError(fatal_err));
                }

                // RecvError - all senders dropped, just exit
                Err(_) => {
                    return Ok(());
                }
            }
        }
    }

    // FatalError should not be passed to this function
    // All errors returned are fatal
    fn handle_event(&mut self, event: DebugEvent) -> Result<()> {
        match event {
            // Handle requests
            DebugEvent::DapRequest(req) => match self.handle_request(&req) {
                Ok(body) => Ok(self.dap_session.send(Response(req.success(body)))?),

                // Recoverable error, send error response
                Err(DebuggerError::RequestFailed(e)) => {
                    let msg = format!("Error handling request: {e}");
                    eprintln!("{msg}");
                    Ok(self.dap_session.send(Response(req.error(&msg)))?)
                }

                // Fatal error, just return
                Err(e) => Err(e),
            },

            // Handle events
            DebugEvent::MsimEvent(event) => match self.handle_msim_event(event) {
                Ok(opt_body) => {
                    if let Some(event_body) = opt_body {
                        self.dap_session.send(Event(event_body))?;
                    }
                    Ok(())
                }

                // Recoverable error, log it and continue
                Err(DebuggerError::RequestFailed(e)) => {
                    eprintln!("Error handling request: {e}");
                    Ok(())
                }

                // Fatal error, just return
                Err(e) => Err(e),
            },
        }
    }

    fn handle_request(&mut self, req: &dap::requests::Request) -> Result<ResponseBody> {
        match &req.command {
            Command::Initialize(args) => self.initialize(args),
            Command::Attach(args) => self.attach(args),
            Command::Launch(args) => self.launch(args),
            Command::ConfigurationDone => self.configuration_done(),
            Command::SetBreakpoints(args) => self.set_breakpoints(args),
            Command::SetExceptionBreakpoints(args) => self.set_exception_breakpoints(args),
            Command::Threads => self.threads(),
            Command::Disconnect(args) => self.disconnect(args),
            _ => Err(DebuggerError::RequestFailed(
                format!("command: {:?}", req.command).into(),
            )),
        }
    }

    fn handle_msim_event(&mut self, event: msim::EventKind) -> Result<Option<dap::events::Event>> {
        match event {
            msim::EventKind::StoppedAt(a) => self.handle_event_stopped_at(a),
        }
    }
}

// DAP request handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    fn initialize(&mut self, args: &InitializeArguments) -> Result<ResponseBody> {
        if let Some(name) = &args.client_name {
            eprintln!("New client: {name}, {}", args.adapter_id);
        }

        Ok(ResponseBody::Initialize(Capabilities {
            supports_configuration_done_request: Some(true),
            ..Default::default() // No extra capabilities advertised
        }))
    }

    fn attach(&mut self, _args: &AttachRequestArguments) -> Result<ResponseBody> {
        eprintln!("Attach request");
        // TODO: move to initialize to be spec-consistent
        self.dap_session
            .send(Event(dap::events::Event::Initialized))?;

        Ok(ResponseBody::Attach)
    }

    fn launch(&mut self, _args: &LaunchRequestArguments) -> Result<ResponseBody> {
        eprintln!("Launch request");
        // TODO: move to initialize to be spec-consistent
        self.dap_session
            .send(Event(dap::events::Event::Initialized))?;
        Ok(ResponseBody::Launch)
    }

    fn configuration_done(&mut self) -> Result<ResponseBody> {
        // TODO: maybe its not universal/shouldn't be to resume on startup

        self.target.resume()?;
        Ok(ResponseBody::ConfigurationDone)
    }

    fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> Result<ResponseBody> {
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
                    TargetError::RequestFailed(e) => {
                        eprintln!("Setting BP {path}:{} failed! ({e})", bp.line);
                    }

                    TargetError::AddressNotFound(path, line) => {
                        eprintln!("Address not found for {path}:{line}");
                    }
                    TargetError::AddressOutOfRange(a) => {
                        let line = bp.line;
                        eprintln!("Address out of range for {path}:{line} ({a})");
                    }
                },
            }

            set_bps.push(bp_info);
        }

        Ok(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
            breakpoints: set_bps,
        }))
    }

    fn set_exception_breakpoints(
        &mut self,
        _args: &SetExceptionBreakpointsArguments,
    ) -> Result<ResponseBody> {
        Ok(ResponseBody::SetExceptionBreakpoints(
            SetExceptionBreakpointsResponse {
                breakpoints: vec![].into(),
            },
        ))
    }

    const fn threads(&mut self) -> Result<ResponseBody> {
        Ok(ResponseBody::Threads(ThreadsResponse { threads: vec![] }))
    }
    const fn disconnect(&mut self, _args: &DisconnectArguments) -> Result<ResponseBody> {
        Ok(ResponseBody::Disconnect)
    }
}

// MSIM event handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    fn handle_event_stopped_at(&mut self, address: Address) -> Result<Option<dap::events::Event>> {
        Ok(Some(dap::events::Event::Stopped(
            dap::events::StoppedEventBody {
                reason: dap::types::StoppedEventReason::Breakpoint,
                description: Some(format!("Stopped at address {address:#x} due to breakpoint")),
                thread_id: None,
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: None,
                hit_breakpoint_ids: None, // TODO: track this
            },
        )))
    }
}

impl From<TargetError> for DebuggerError {
    fn from(error: TargetError) -> Self {
        match error {
            TargetError::SessionLost => Self::SessionLost,
            e => Self::RequestFailed(Box::new(e)),
        }
    }
}
