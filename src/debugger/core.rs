//! Core logic of the debugger, including the main event loop and
//! the dispatching of events and requests to their respective handlers.
use super::{DebuggerError, DebuggerError::RequestFailed, Result};
use crate::adapter::Session;
use crate::target::{DebugTarget, TargetError};
use crate::{Address, CpuId, DebugEvent, DebugEventReceiver, LineNo, msim};
use dap::base_message::Sendable::{Event, Response};
use dap::prelude::{Command, ResponseBody};
use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

/// Main debugger struct, containing the state of the debugging session and
/// the main event loop.
/// It is responsible for receiving events from the target and requests from the DAP client,
/// dispatching them to the appropriate handlers, and sending responses and events back to the client.
/// It operates at the highest level of the debugger, coordinating between the DAP session, the target session,
/// and the internal state of the debugger.
pub struct Debugger<T: DebugTarget> {
    pub(super) receiver: DebugEventReceiver,
    pub(super) dap_session: Session,
    pub(super) target: T,
    pub(super) bp_registry: BpRegistry,
    pub(super) cpu_registry: CpuRegistry,
    pub(super) step_bp: HashMap<CpuId, Address>, // Address of pending step breakpoint
}

/// Breakpoint ID
pub type BpId = u32;
/// Thread ID
pub type ThreadId = i64;
/// Stack frame ID
pub type FrameId = i64;
/// Variable reference ID
pub type VarRef = i64;

/// Registry for breakpoint state
pub struct BpRegistry {
    next_id: BpId,
    ids: HashMap<(PathBuf, LineNo), BpId>,
}

/// Registry for CPU and thread state, mapping between target CPU IDs and DAP thread IDs
pub(super) struct CpuRegistry;

/// Memory reference, encoding either a physical address or a virtual address with CPU ID
pub(super) enum MemoryRef {
    /// Physical memory reference, containing the physical address
    Physical(Address),
    /// Virtual memory reference, containing the CPU ID and the virtual address
    Virtual(CpuId, Address),
}

/// Kind of a variable scope
#[derive(Debug)]
pub(super) enum VarScopeKind {
    /// General-purpose registers for a specific CPU
    GeneralRegisters(CpuId),
    /// Control and status registers for a specific CPU
    CsrRegisters(CpuId),
}

/// Action to perform after sending a response to a DAP request,
/// allowingthe handler to specify additional events to send or to disconnect the session.
#[allow(clippy::large_enum_variant)]
pub(super) enum PostAction {
    /// Send an additional event to the DAP client after sending the response
    SendEvent(dap::events::Event),
    /// Disconnect the DAP session after sending the response
    Disconnect,
}

/// Result of handling a DAP request, containing the response body to send and an optional [`PostAction`] to perform.
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
            step_bp: HashMap::new(),
        }
    }

    /// Main event loop of the debugger, dispatching incoming events and requests to their respective handlers
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

    /// Handle a single debug event, which can be either a DAP request or a target event.
    /// `FatalError` should not be passed to this function
    /// All errors returned are fatal
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

    /// Handle a DAP request, dispatching to the appropriate handler based on the command.
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
            Command::ReadMemory(args) => self.read_memory(args),
            Command::Source(args) => self.source(args),

            _ => Err(DebuggerError::RequestFailed(
                format!("Unhandled command: {:?}", req.command).into(),
            )),
        }
    }

    /// Handle an event from the target, dispatching to the appropriate handler based on the event type.
    fn handle_msim_event(&mut self, event: msim::Event) -> Result<Option<dap::events::Event>> {
        match event {
            msim::Event::Terminated => self.handle_event_terminated(),
            msim::Event::StoppedAt(cpu, address, reason) => {
                self.handle_event_stopped_at(cpu, address, reason)
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

    /// Get the breakpoint ID for the given source location, creating a new one if it doesn't exist.
    /// This is idempotent.
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

impl MemoryRef {
    /// Delimiter used in the string representation of memory references
    const DELIM: char = ':';

    /// Get the address contained in the memory reference, regardless of whether it is physical or virtual.
    pub const fn address(&self) -> Address {
        match self {
            Self::Physical(addr) | Self::Virtual(_, addr) => *addr,
        }
    }

    /// Parse a memory reference from a string
    pub fn parse(string: &str) -> Result<Self> {
        let mut parts = string.split(Self::DELIM);
        let kind = parts.next().ok_or_else(|| {
            RequestFailed(format!("Invalid memory reference (missing kind): {string}").into())
        })?;

        match kind {
            "phys" => {
                let addr = parts
                    .next()
                    .ok_or_else(|| {
                        RequestFailed(
                            format!(
                                "Invalid physical memory reference (missing address): {string}"
                            )
                            .into(),
                        )
                    })
                    .and_then(|s| Self::parse_hex(s, "address"))?;
                Ok(Self::Physical(addr))
            }

            "virt" => {
                let cpu_id = parts
                    .next()
                    .ok_or_else(|| {
                        RequestFailed(
                            format!("Invalid virtual memory reference (missing CPU ID): {string}")
                                .into(),
                        )
                    })
                    .and_then(|s| {
                        s.parse::<CpuId>().map_err(|e| {
                        RequestFailed(format!("Invalid virtual memory reference (invalid CPU ID): {string}: {e}").into()) })
                    })?;

                let addr = parts
                    .next()
                    .ok_or_else(|| {
                        RequestFailed(
                            format!("Invalid virtual memory reference (missing address): {string}")
                                .into(),
                        )
                    })
                    .and_then(|s| Self::parse_hex(s, "address"))?;
                Ok(Self::Virtual(cpu_id, addr))
            }

            _ => Err(RequestFailed(
                format!("Invalid memory reference (unknown kind): {string}").into(),
            )),
        }
    }

    fn parse_hex(string: &str, ctx: &str) -> Result<u64> {
        u64::from_str_radix(string.trim_start_matches("0x"), 16).map_err(|e| {
            RequestFailed(
                format!("Invalid memory reference (invalid {ctx}) '{string}': {e}").into(),
            )
        })
    }
}

impl Display for MemoryRef {
    /// String representation of memory references, used for encoding them in DAP variable references.
    /// Examples:
    /// - Physical memory reference: `phys:0x1234`
    /// - Virtual memory reference: `virt:0:0x5678` (CPU ID 0, address 0x5678)
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Physical(addr) => write!(f, "phys:{addr:#x}"),
            Self::Virtual(cpu_id, addr) => write!(f, "virt:{cpu_id}:{addr:#x}"),
        }
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
