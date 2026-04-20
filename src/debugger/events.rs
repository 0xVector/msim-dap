use super::{Debugger, Result};
use crate::Address;
use crate::target::DebugTarget;
use dap::base_message::Sendable;

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

    pub(super) fn handle_event_stopped_at(&mut self, address: Address) -> EventResult {
        eprintln!("Stopped at address {address:#x}");
        let mut hits = vec![];
        let mut message = format!("Stopped at {address:#x}");
        if let Some((source, line)) = self.target.resolve_code_bp(address) {
            hits.push(i64::from(self.bp_registry.get_id(source, line)));
            message = format!(
                "Stopped at {}:{line} (ID {:?})",
                source.display(),
                hits.last()
            );
            eprintln!("{message}");
        }

        Ok(Some(dap::events::Event::Stopped(
            dap::events::StoppedEventBody {
                reason: dap::types::StoppedEventReason::Breakpoint,
                description: Some(message),
                thread_id: None,
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: None,
                hit_breakpoint_ids: (!hits.is_empty()).then_some(hits),
            },
        )))
    }
}
