use super::{Debugger, Result};
use crate::Address;
use crate::target::DebugTarget;

// MSIM event handling
// To make handlers consistent silence linter:
#[allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_ref_mut
)]
impl<T: DebugTarget> Debugger<T> {
    pub(super) fn handle_event_stopped_at(
        &mut self,
        address: Address,
    ) -> Result<Option<dap::events::Event>> {
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
