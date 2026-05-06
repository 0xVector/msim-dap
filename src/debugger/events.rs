//! MSIM event handling and conversion to DAP events.
use super::{Debugger, Result};
use crate::msim::StoppedAtReason;
use crate::target::DebugTarget;
use crate::{Address, CpuId};
use dap::base_message::Sendable;
use dap::types::StoppedEventReason;

type EventResult = Result<Option<dap::events::Event>>;

// MSIM event handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    /// Handle a MSIM `Terminated`"
    pub(super) fn handle_event_terminated(&mut self) -> EventResult {
        // TODO: we could allow returning multiple events (like in request handlers)
        self.dap_session
            .send(Sendable::Event(dap::events::Event::Terminated(None)))?;

        Ok(Some(dap::events::Event::Exited(
            dap::events::ExitedEventBody { exit_code: 0 }, // TODO: track exit code?
        )))
    }

    /// Handle a MSIM "`StoppedAt`"
    pub(super) fn handle_event_stopped_at(
        &mut self,
        cpu: CpuId,
        address: Address,
        reason: StoppedAtReason,
    ) -> EventResult {
        let mut hits = vec![];

        let source_desc = match self.target.resolve_address(address) {
            Some((path, line)) => {
                format!("{}:{line}", path.display())
            }
            None => "unknown file".to_string(),
        };

        let mut bp_message = None;

        // Step breakpoints handling
        let dap_reason = if self.step_bp.get(&cpu) == Some(&address) {
            self.step_bp.remove(&cpu);
            if let Some((path, line)) = self.target.resolve_address(address) {
                self.target
                    .remove_code_bp(path.to_owned().as_path(), line)
                    .ok(); // best effort cleanup, not critical if it fails
            }
            StoppedEventReason::Step
        } else {
            match reason {
                StoppedAtReason::Paused => StoppedEventReason::Pause,
                StoppedAtReason::Breakpoint => {
                    if let Some((path, line)) = self.target.resolve_code_bp(address) {
                        let id = i64::from(self.bp_registry.get_id(path, line));
                        hits.push(id);
                        bp_message = Some(format!("(hit BP {id})"));
                    }
                    StoppedEventReason::Breakpoint
                }
                StoppedAtReason::StepComplete => StoppedEventReason::Step,
                StoppedAtReason::Interrupt => StoppedEventReason::Exception,
            }
        };
        let message = format!(
            "Stopped at {address:#x} ({source_desc}) due to {dap_reason:?}{}",
            bp_message.map(|m| format!(" {m}")).unwrap_or_default()
        );
        eprintln!("{message}");

        Ok(Some(dap::events::Event::Stopped(
            dap::events::StoppedEventBody {
                reason: dap_reason,
                description: Some(message),
                thread_id: Some(self.cpu_registry.cpu_to_thread_id(cpu)),
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: Some(true),
                hit_breakpoint_ids: (!hits.is_empty()).then_some(hits),
            },
        )))
    }
}
