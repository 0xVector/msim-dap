use super::{DebuggerError, Result};
use crate::adapter::Session;
use crate::target::{DebugTarget, TargetError};
use crate::{Address, DebugEvent, DebugEventReceiver, LineNo, msim};
use dap::base_message::Sendable::{Event, Response};
use dap::prelude::{Command, ResponseBody};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Debugger<T: DebugTarget> {
    pub(super) receiver: DebugEventReceiver,
    pub(super) dap_session: Session,
    pub(super) target: T,
    pub(super) bp_registry: BpRegistry,
    pub(super) last_stopped_at: Option<Address>,
}

pub type BpId = u32;

pub struct BpRegistry {
    next_id: BpId,
    ids: HashMap<(PathBuf, LineNo), BpId>,
}

#[allow(clippy::large_enum_variant)]
pub(super) enum PostAction {
    SendEvent(dap::events::Event),
    Disconnect,
}

pub(super) struct HandlerAction {
    pub body: ResponseBody,
    pub post_action: Option<PostAction>,
}

impl<T: DebugTarget> Debugger<T> {
    pub fn new(receiver: DebugEventReceiver, dap_session: Session, msim_session: T) -> Self {
        Self {
            receiver,
            dap_session,
            target: msim_session,
            bp_registry: BpRegistry::new(),
            last_stopped_at: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            match self.receiver.recv() {
                Ok(Ok(debug_event)) => match self.handle(debug_event) {
                    Err(DebuggerError::DapDisconnected) => return Ok(()),
                    other => other?,
                },

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
    fn handle(&mut self, event: DebugEvent) -> Result<()> {
        match event {
            // Handle requests
            DebugEvent::DapRequest(req) => match self.handle_dap_request(&req) {
                Ok(HandlerAction { body, post_action }) => {
                    self.dap_session.send(Response(req.success(body)))?;
                    post_action.map_or(Ok(()), |action| action.execute(&self.dap_session))
                }

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
                    eprintln!("Error handling event: {e}");
                    Ok(())
                }

                // Fatal error, just return
                Err(e) => Err(e),
            },
        }
    }

    fn handle_dap_request(&mut self, req: &dap::requests::Request) -> Result<HandlerAction> {
        match &req.command {
            Command::Initialize(args) => self.initialize(args),
            Command::Attach(args) => self.attach(args),
            Command::Launch(args) => self.launch(args),
            Command::ConfigurationDone => self.configuration_done(),
            Command::SetBreakpoints(args) => self.set_breakpoints(args),
            Command::SetExceptionBreakpoints(args) => self.set_exception_breakpoints(args),
            Command::Threads => self.threads(),
            Command::Disconnect(args) => self.disconnect(args),
            Command::StackTrace(args) => self.stack_trace(args),
            Command::Scopes(args) => self.scopes(args),
            Command::Continue(args) => self.resume(args),
            Command::Pause(args) => self.pause(args),
            _ => Err(DebuggerError::RequestFailed(
                format!("Unhandled command: {:?}", req.command).into(),
            )),
        }
    }

    fn handle_msim_event(&mut self, event: msim::Event) -> Result<Option<dap::events::Event>> {
        match event {
            msim::Event::Exited => self.handle_event_exited(),
            msim::Event::StoppedAt(address, reason) => {
                self.handle_event_stopped_at(address, reason)
            }
        }
    }
}

impl BpRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            ids: HashMap::default(),
        }
    }

    pub fn get_id(&mut self, path: &Path, line: LineNo) -> BpId {
        *self
            .ids
            .entry((path.to_path_buf(), line))
            .or_insert_with(|| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
    }
}

impl PostAction {
    fn execute(self, session: &Session) -> Result<()> {
        match self {
            Self::SendEvent(event) => Ok(session.send(Event(event))?),
            Self::Disconnect => Err(DebuggerError::DapDisconnected),
        }
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
