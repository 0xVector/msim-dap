use super::{Debugger, Result};
use crate::Address;
use crate::msim::StoppedAtReason;
use crate::target::DebugTarget;
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
    pub(super) fn handle_event_exited(&mut self) -> EventResult {
        // TODO: we could allow returning multiple events (like in request handlers)
        self.dap_session
            .send(Sendable::Event(dap::events::Event::Terminated(None)))?;

        Ok(Some(dap::events::Event::Exited(
            dap::events::ExitedEventBody { exit_code: 0 }, // TODO: track exit code?
        )))
    }

    pub(super) fn handle_event_stopped_at(
        &mut self,
        address: Address,
        reason: StoppedAtReason,
    ) -> EventResult {
        self.last_stopped_at = Some(address);
        let mut hits = vec![];

        let source_desc = match self.target.resolve_address(address) {
            Some((path, line)) => {
                format!("{}:{line}", path.display())
            }
            None => "unknown file".to_string(),
        };
        let mut message = format!("Stopped at {address:#x} ({source_desc}) due to {reason:?}");

        let reason = match reason {
            StoppedAtReason::Paused => StoppedEventReason::Pause,
            StoppedAtReason::Breakpoint => {
                if let Some((path, line)) = self.target.resolve_code_bp(address) {
                    let id = i64::from(self.bp_registry.get_id(path, line));
                    hits.push(id);
                    message = format!("{message} (hit BP {id})");
                }
                StoppedEventReason::Breakpoint
            }
            StoppedAtReason::StepComplete => StoppedEventReason::Step,
            StoppedAtReason::Interrupt => StoppedEventReason::Exception,
        };
        eprintln!("{message}");

        Ok(Some(dap::events::Event::Stopped(
            dap::events::StoppedEventBody {
                reason,
                description: Some(message),
                thread_id: Some(1), // TODO: track (CPU ID)
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: Some(true),
                hit_breakpoint_ids: (!hits.is_empty()).then_some(hits),
            },
        )))
    }
}
