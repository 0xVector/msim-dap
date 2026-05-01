use super::{DebuggerError, Result};
use crate::adapter::Session;
use crate::debugger::DebuggerError::RequestFailed;
use crate::target::{DebugTarget, TargetError};
use crate::{Address, CpuId, DebugEvent, DebugEventReceiver, LineNo, msim};
use dap::base_message::Sendable::{Event, Response};
use dap::prelude::{Command, ResponseBody};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Debugger<T: DebugTarget> {
    pub(super) receiver: DebugEventReceiver,
    pub(super) dap_session: Session,
    pub(super) target: T,
    pub(super) bp_registry: BpRegistry,
    pub(super) cpu_registry: CpuRegistry,
    pub(super) last_stopped_at: Option<Address>,
    pub(super) step_bp: Option<Address>, // Address of pending step breakpoint
}

pub type BpId = u32;
pub type ThreadId = i64;
pub type FrameId = i64;
pub type VarRef = i64;

pub struct BpRegistry {
    next_id: BpId,
    ids: HashMap<(PathBuf, LineNo), BpId>,
}

pub(super) struct CpuRegistry {}

#[derive(Debug)]
pub(super) enum VarScopeKind {
    GeneralRegisters(CpuId),
    CsrRegisters(CpuId),
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
            cpu_registry: CpuRegistry::new(),
            last_stopped_at: None,
            step_bp: None,
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
            Command::Next(args) => self.next(args),
            Command::StepIn(args) => self.step_in(args),
            Command::StepOut(args) => self.step_out(args),
            Command::Variables(args) => self.variables(args),
            Command::SetVariable(args) => self.set_variable(args),

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

impl CpuRegistry {
    pub const fn new() -> Self {
        Self {}
    }

    /// Convert a CPU ID from the target to a thread ID for DAP.
    #[allow(clippy::unused_self)] // will likely need to track more info about CPUs in the future
    pub const fn cpu_to_thread_id(&self, cpu_id: CpuId) -> ThreadId {
        (cpu_id + 1).cast_signed()
    }

    /// Convert a thread ID from DAP to a CPU ID for the target.
    #[allow(clippy::unused_self)]
    pub fn thread_to_cpu_id(&self, thread_id: ThreadId) -> Result<CpuId> {
        if thread_id > 0 {
            Ok((thread_id - 1).cast_unsigned())
        } else {
            Err(RequestFailed(
                format!("Invalid thread ID (CPU ID) {thread_id}").into(),
            ))
        }
    }

    /// Get the stack frame ID for the given thread ID.
    #[allow(clippy::unused_self)]
    pub const fn thread_to_frame_id(&self, thread_id: ThreadId) -> FrameId {
        thread_id
    }

    /// Get the thread ID for the given stack frame ID.
    #[allow(clippy::unused_self)]
    pub const fn frame_to_thread_id(&self, frame_id: FrameId) -> ThreadId {
        frame_id
    }

    /// Get the general register scope ID for the given stack frame ID.
    #[allow(clippy::unused_self)]
    pub const fn reg_scope_var_ref(&self, frame_id: FrameId) -> VarRef {
        frame_id * 10 + 1
    }

    /// Get the CSR register scope ID for the given stack frame ID.
    #[allow(clippy::unused_self)]
    pub const fn csr_scope_var_ref(&self, frame_id: FrameId) -> VarRef {
        frame_id * 10 + 2
    }

    /// Resolve a variable reference to a scope kind
    pub fn resolve_var_ref(&self, var_ref: VarRef) -> Result<VarScopeKind> {
        let frame_id: FrameId = var_ref / 10;
        let scope_kind = var_ref % 10;
        let cpu_id = self.thread_to_cpu_id(self.frame_to_thread_id(frame_id))?;
        let scope = match scope_kind {
            1 => VarScopeKind::GeneralRegisters(cpu_id),
            2 => VarScopeKind::CsrRegisters(cpu_id),
            _ => {
                return Err(RequestFailed(
                    format!("Invalid variables reference: {var_ref}").into(),
                ));
            }
        };
        Ok(scope)
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
