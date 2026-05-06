use super::core::{HandlerAction, MemoryRef, PostAction, PostAction::Disconnect, VarScopeKind};
use super::{Debugger, DebuggerError, DebuggerError::RequestFailed, Result};
use crate::target::{DebugTarget, TargetError};
use crate::{CpuId, LineNo};
use base64::{Engine, engine::general_purpose::STANDARD};
use dap::prelude::ResponseBody;
use dap::requests::{
    AttachRequestArguments, ContinueArguments, DisconnectArguments, InitializeArguments,
    LaunchRequestArguments, NextArguments, PauseArguments, ReadMemoryArguments, ScopesArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments, SourceArguments,
    StackTraceArguments, StepInArguments, StepOutArguments,
};
use dap::responses::{
    SetBreakpointsResponse, SetExceptionBreakpointsResponse, StackTraceResponse, ThreadsResponse,
};
use dap::types::{
    Breakpoint, Capabilities, Scope, ScopePresentationhint, Source, StackFrame, Thread,
};
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
                supports_set_variable: Some(true),
                supports_read_memory_request: Some(true),
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
        let path = args.source.path.as_deref().ok_or(RequestFailed(
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

                    // These errors should not occur when setting code breakpoints
                    e => {
                        eprintln!("Unexpected error setting BP at {path}:{}: {e}", bp.line);
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
        let cpu_count: CpuId = self.target.cpu_count().unwrap_or(1);

        Ok(HandlerAction {
            body: ResponseBody::Threads(ThreadsResponse {
                threads: (0..cpu_count)
                    .map(|cpu_id| Thread {
                        id: self.cpu_registry.cpu_to_thread_id(cpu_id),
                        name: format!("CPU {cpu_id}"),
                    })
                    .collect(),
            }),
            post_action: None,
        })
    }

    pub(super) fn disconnect(&mut self, _args: &DisconnectArguments) -> HandlerResult {
        self.target.terminate().ok(); // Best effort to stop target, ignore errors since we're disconnecting anyway

        Ok(HandlerAction {
            body: ResponseBody::Disconnect,
            post_action: Some(Disconnect), // Signal to stop the event loop
        })
    }

    pub(super) fn stack_trace(&mut self, args: &StackTraceArguments) -> HandlerResult {
        let cpu_id = self.cpu_registry.thread_to_cpu_id(args.thread_id)?;
        let address = self.target.read_pc(cpu_id)?;

        let (path, line) = self
            .target
            .resolve_address(address)
            .map_or((None, 0), |(path, line)| {
                (Some(path.to_string_lossy().into_owned()), line)
            });
        eprintln!(
            "Stack trace requested, last stopped at: {address} ({}:{})",
            path.as_deref().unwrap_or("<unknown>"),
            line
        );

        // Defaults for missing sources
        let presentation_hint = if path.is_none() {
            Some(dap::types::PresentationHint::DeEmphasize)
        } else {
            None
        };
        let name = if path.is_none() {
            Some(format!(
                "Address {address:#x}, unknown (missing debug info)"
            ))
        } else {
            None
        };

        Ok(HandlerAction {
            body: ResponseBody::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: self.cpu_registry.thread_to_frame_id(args.thread_id),
                    name: "<unknown>".into(),
                    source: Some(Source {
                        path,
                        name,
                        presentation_hint,
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

    pub(super) fn scopes(&mut self, args: &ScopesArguments) -> HandlerResult {
        eprintln!(
            "Scopes requested for frame {} (CPU {})",
            args.frame_id,
            self.cpu_registry
                .thread_to_cpu_id(self.cpu_registry.frame_to_thread_id(args.frame_id))?
        );

        Ok(HandlerAction {
            body: ResponseBody::Scopes(dap::responses::ScopesResponse {
                scopes: vec![
                    Scope {
                        name: "Registers".to_string(),
                        presentation_hint: Some(ScopePresentationhint::Registers),
                        variables_reference: self.cpu_registry.reg_scope_var_ref(args.frame_id),
                        expensive: false,
                        ..Default::default()
                    },
                    Scope {
                        name: "Control and Status registers (CSR)".to_string(),
                        presentation_hint: Some(ScopePresentationhint::Registers),
                        variables_reference: self.cpu_registry.csr_scope_var_ref(args.frame_id),
                        expensive: false,
                        ..Default::default()
                    },
                ],
            }),
            post_action: None,
        })
    }

    pub(super) fn variables(&mut self, args: &dap::requests::VariablesArguments) -> HandlerResult {
        let scope = self
            .cpu_registry
            .resolve_var_ref(args.variables_reference)?;

        let (cpu, registers) = match scope {
            VarScopeKind::GeneralRegisters(cpu) => (cpu, self.target.read_general_regs(cpu)?),
            VarScopeKind::CsrRegisters(cpu) => (cpu, self.target.read_csrs(cpu)?),
        };
        let variables = registers
            .into_iter()
            .map(|reg| dap::types::Variable {
                name: reg.name.to_string(),
                value: format!("{:#x}", reg.value),
                variables_reference: 0, // No nested variables for registers
                // type_field: Some("u64".into()), // TODO: figure out what looks best in UI
                presentation_hint: Some(dap::types::VariablePresentationHint {
                    kind: Some(dap::types::VariablePresentationHintKind::String(
                        "register".into(),
                    )),
                    attributes: None,
                    visibility: None,
                    lazy: None,
                }),
                memory_reference: Some(MemoryRef::Virtual(cpu, reg.value).to_string()),
                ..Default::default()
            })
            .collect();

        eprintln!("Variables returned for scope {scope:?}");
        Ok(HandlerAction {
            body: ResponseBody::Variables(dap::responses::VariablesResponse { variables }),
            post_action: None,
        })
    }

    pub(super) fn set_variable(
        &mut self,
        args: &dap::requests::SetVariableArguments,
    ) -> HandlerResult {
        let scope = self
            .cpu_registry
            .resolve_var_ref(args.variables_reference)?;

        let value = match &args.value {
            v if v.starts_with("0x") => u64::from_str_radix(&v[2..], 16).map_err(|_| {
                RequestFailed(format!("Invalid hexadecimal value: {}", args.value).into())
            })?,

            v if v.starts_with("0b") => u64::from_str_radix(&v[2..], 2).map_err(|_| {
                RequestFailed(format!("Invalid binary value: {}", args.value).into())
            })?,

            v => v.parse::<u64>().map_err(|_| {
                RequestFailed(format!("Invalid decimal value: {}", args.value).into())
            })?,
        };

        match scope {
            VarScopeKind::GeneralRegisters(cpu) => {
                self.target.write_general_reg(cpu, &args.name, value)?;
            }
            VarScopeKind::CsrRegisters(cpu) => self.target.write_csr(cpu, &args.name, value)?,
        }

        Ok(HandlerAction {
            body: ResponseBody::SetVariable(dap::responses::SetVariableResponse {
                value: format!("{value:#x}"),
                type_field: Some("register".into()),
                ..Default::default()
            }),
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

    pub(super) fn next(&mut self, args: &NextArguments) -> HandlerResult {
        // Next means step over in DAP
        let cpu_id = self.cpu_registry.thread_to_cpu_id(args.thread_id)?;
        let address = self.target.read_pc(cpu_id)?;

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

            self.step_bp.insert(cpu_id, next_address);

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

    pub(super) fn step_in(&mut self, args: &StepInArguments) -> HandlerResult {
        self.target
            .step_by(self.cpu_registry.thread_to_cpu_id(args.thread_id)?, 1)?;

        Ok(HandlerAction {
            body: ResponseBody::StepIn,
            post_action: None,
        })
    }

    pub(super) fn step_out(&mut self, args: &StepOutArguments) -> HandlerResult {
        self.target
            .step_by(self.cpu_registry.thread_to_cpu_id(args.thread_id)?, 1)?; // TODO: actual solution

        Ok(HandlerAction {
            body: ResponseBody::StepOut,
            post_action: None,
        })
    }

    pub(super) fn read_memory(&mut self, args: &ReadMemoryArguments) -> HandlerResult {
        let length =
            usize::try_from(args.count).map_err(|_| RequestFailed("Invalid byte count".into()))?;
        let offset = args.offset.unwrap_or(0);

        let mem_ref = MemoryRef::parse(&args.memory_reference)?;
        let address = (mem_ref.address().cast_signed() + offset).cast_unsigned();

        let data = match mem_ref {
            MemoryRef::Physical(_base) => self.target.read_phys_memory(address, length)?,
            MemoryRef::Virtual(cpu, _base) => self.target.read_virt_memory(cpu, address, length)?,
        };

        eprintln!(
            "Read of {length} bytes from memory requested: got {} B at {address:#x} (offset {offset:#x}) (ref {})",
            data.len(),
            args.memory_reference
        );
        Ok(HandlerAction {
            body: ResponseBody::ReadMemory(dap::responses::ReadMemoryResponse {
                address: format!("{address:#x}"),
                unreadable_bytes: None,
                data: Some(STANDARD.encode(data)),
            }),
            post_action: None,
        })
    }

    pub(super) fn source(&mut self, _args: &SourceArguments) -> HandlerResult {
        Ok(HandlerAction {
            body: ResponseBody::Source(dap::responses::SourceResponse {
                content: "<unknown file>\
                (No debug info available, are we in the userspace app?)"
                    .to_string(),
                mime_type: None,
            }),
            post_action: None,
        })
    }
}
